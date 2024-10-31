use cosmwasm_schema::cw_serde;
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_json_binary, Addr, Binary, CosmosMsg, Deps, DepsMut, Empty, Env, MessageInfo, Order,
    Response, StdError, StdResult, SubMsg, Uint128, WasmMsg,
};
use cw2::set_contract_version;
use cw_denom::CheckedDenom;
use cw_paginate_storage::paginate_map_values;
use cw_storage_plus::Map;
use dao_pre_propose_base::{
    error::PreProposeError, msg::ExecuteMsg as ExecuteBase, state::PreProposeContract,
};
use dao_voting::approval::{ApprovalProposalStatus, ApproverProposeMessage};
use dao_voting::deposit::{CheckedDepositInfo, DepositRefundPolicy};
use dao_voting::proposal::SingleChoiceProposeMsg as ProposeMsg;
use dao_voting::voting::{SingleChoiceAutoVote, Vote};

use crate::msg::{
    ExecuteExt, ExecuteMsg, InstantiateExt, InstantiateMsg, MigrateMsg, ProposeMessage,
    ProposeMessageInternal, QueryExt, QueryMsg,
};
use crate::state::{
    advance_approval_id, Proposal, APPROVER, COMPLETED_PROPOSALS,
    CREATED_PROPOSAL_TO_COMPLETED_PROPOSAL, PENDING_PROPOSALS,
};

pub(crate) const CONTRACT_NAME: &str = "crates.io:dao-pre-propose-approval-single";
pub(crate) const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

type PrePropose = PreProposeContract<InstantiateExt, ExecuteExt, QueryExt, Empty, ProposeMessage>;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, PreProposeError> {
    let approver = deps.api.addr_validate(&msg.extension.approver)?;
    APPROVER.save(deps.storage, &approver)?;

    let resp = PrePropose::default().instantiate(deps.branch(), env, info, msg)?;
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(resp.add_attribute("approver", approver.to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, PreProposeError> {
    match msg {
        ExecuteMsg::Propose { msg } => execute_propose(deps, env, info, msg),

        ExecuteMsg::AddProposalSubmittedHook { address } => {
            execute_add_approver_hook(deps, info, address)
        }
        ExecuteMsg::RemoveProposalSubmittedHook { address } => {
            execute_remove_approver_hook(deps, info, address)
        }

        ExecuteMsg::Extension { msg } => match msg {
            ExecuteExt::Approve { id } => execute_approve(deps, info, id),
            ExecuteExt::Reject { id } => execute_reject(deps, info, id),
            ExecuteExt::UpdateApprover { address } => execute_update_approver(deps, info, address),
        },
        // Default pre-propose-base behavior for all other messages
        _ => PrePropose::default().execute(deps, env, info, msg),
    }
}

pub fn execute_propose(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ProposeMessage,
) -> Result<Response, PreProposeError> {
    let pre_propose_base = PrePropose::default();
    let config = pre_propose_base.config.load(deps.storage)?;

    pre_propose_base.check_can_submit(deps.as_ref(), info.sender.clone())?;

    // Take deposit, if configured.
    let deposit_messages = if let Some(ref deposit_info) = config.deposit_info {
        deposit_info.check_native_deposit_paid(&info)?;
        deposit_info.get_take_deposit_messages(&info.sender, &env.contract.address)?
    } else {
        vec![]
    };

    let approval_id = advance_approval_id(deps.storage)?;

    let propose_msg_internal = match msg {
        ProposeMessage::Propose {
            title,
            description,
            msgs,
            vote,
        } => ProposeMsg {
            title,
            description,
            msgs,
            proposer: Some(info.sender.to_string()),
            vote,
        },
    };

    // Prepare proposal submitted hooks msg to notify approver.  Make
    // a proposal on the approver DAO to approve this pre-proposal
    let hooks_msgs =
        pre_propose_base
            .proposal_submitted_hooks
            .prepare_hooks(deps.storage, |a| {
                let execute_msg = WasmMsg::Execute {
                    contract_addr: a.into_string(),
                    msg: to_json_binary(&ExecuteBase::<ApproverProposeMessage, Empty>::Propose {
                        msg: ApproverProposeMessage::Propose {
                            title: propose_msg_internal.title.clone(),
                            description: propose_msg_internal.description.clone(),
                            approval_id,
                        },
                    })?,
                    funds: vec![],
                };
                Ok(SubMsg::new(execute_msg))
            })?;

    let approver = APPROVER.load(deps.storage)?;

    // Save the proposal and its information as pending.
    PENDING_PROPOSALS.save(
        deps.storage,
        approval_id,
        &Proposal {
            status: ApprovalProposalStatus::Pending {},
            approval_id,
            approver: approver.clone(),
            proposer: info.sender,
            msg: propose_msg_internal,
            deposit: config.deposit_info,
        },
    )?;

    Ok(Response::default()
        .add_messages(deposit_messages)
        .add_submessages(hooks_msgs)
        .add_attribute("method", "pre-propose")
        .add_attribute("id", approval_id.to_string())
        .add_attribute("approver", approver.to_string()))
}

pub fn execute_approve(
    deps: DepsMut,
    info: MessageInfo,
    id: u64,
) -> Result<Response, PreProposeError> {
    // Load proposal and send propose message to the proposal module
    let proposal = PENDING_PROPOSALS.may_load(deps.storage, id)?;
    match proposal {
        Some(proposal) => {
            // Check sender is the approver
            if proposal.approver != info.sender {
                return Err(PreProposeError::Unauthorized {});
            }

            let proposal_module = PrePropose::default().proposal_module.load(deps.storage)?;

            // Snapshot the deposit for the proposal that we're about
            // to create.
            let proposal_id = deps.querier.query_wasm_smart(
                &proposal_module,
                &dao_interface::proposal::Query::NextProposalId {},
            )?;
            PrePropose::default().deposits.save(
                deps.storage,
                proposal_id,
                &(proposal.deposit.clone(), proposal.proposer.clone()),
            )?;

            let propose_messsage = WasmMsg::Execute {
                contract_addr: proposal_module.into_string(),
                msg: to_json_binary(&ProposeMessageInternal::Propose(proposal.msg.clone()))?,
                funds: vec![],
            };

            COMPLETED_PROPOSALS.save(
                deps.storage,
                id,
                &Proposal {
                    status: ApprovalProposalStatus::Approved {
                        created_proposal_id: proposal_id,
                    },
                    approval_id: proposal.approval_id,
                    approver: proposal.approver,
                    proposer: proposal.proposer,
                    msg: proposal.msg,
                    deposit: proposal.deposit,
                },
            )?;
            CREATED_PROPOSAL_TO_COMPLETED_PROPOSAL.save(deps.storage, proposal_id, &id)?;
            PENDING_PROPOSALS.remove(deps.storage, id);

            Ok(Response::default()
                .add_message(propose_messsage)
                .add_attribute("method", "proposal_approved")
                .add_attribute("approval_id", id.to_string())
                .add_attribute("proposal_id", proposal_id.to_string()))
        }
        None => Err(PreProposeError::ProposalNotFound {}),
    }
}

pub fn execute_reject(
    deps: DepsMut,
    info: MessageInfo,
    id: u64,
) -> Result<Response, PreProposeError> {
    let Proposal {
        approval_id,
        approver,
        proposer,
        msg,
        deposit,
        ..
    } = PENDING_PROPOSALS
        .may_load(deps.storage, id)?
        .ok_or(PreProposeError::ProposalNotFound {})?;

    // Check sender is the approver
    if approver != info.sender {
        return Err(PreProposeError::Unauthorized {});
    }

    COMPLETED_PROPOSALS.save(
        deps.storage,
        id,
        &Proposal {
            status: ApprovalProposalStatus::Rejected {},
            approval_id,
            approver,
            proposer: proposer.clone(),
            msg: msg.clone(),
            deposit: deposit.clone(),
        },
    )?;
    PENDING_PROPOSALS.remove(deps.storage, id);

    let messages = if let Some(ref deposit_info) = deposit {
        // Refund can be issued if proposal if deposits are always
        // refunded. `OnlyPassed` and `Never` refund deposit policies
        // do not apply here.
        if deposit_info.refund_policy == DepositRefundPolicy::Always {
            deposit_info.get_return_deposit_message(&proposer)?
        } else {
            // If the proposer doesn't get the deposit, the DAO does.
            let dao = PrePropose::default().dao.load(deps.storage)?;
            deposit_info.get_return_deposit_message(&dao)?
        }
    } else {
        vec![]
    };

    Ok(Response::default()
        .add_attribute("method", "proposal_rejected")
        .add_attribute("proposal", id.to_string())
        .add_attribute("deposit_info", to_json_binary(&deposit)?.to_string())
        .add_messages(messages))
}

pub fn execute_update_approver(
    deps: DepsMut,
    info: MessageInfo,
    address: String,
) -> Result<Response, PreProposeError> {
    // Check sender is the approver
    let approver = APPROVER.load(deps.storage)?;
    if approver != info.sender {
        return Err(PreProposeError::Unauthorized {});
    }

    // Validate address and save new approver
    let addr = deps.api.addr_validate(&address)?;
    APPROVER.save(deps.storage, &addr)?;

    Ok(Response::default())
}

pub fn execute_add_approver_hook(
    deps: DepsMut,
    info: MessageInfo,
    address: String,
) -> Result<Response, PreProposeError> {
    let pre_propose_base = PrePropose::default();

    let dao = pre_propose_base.dao.load(deps.storage)?;
    let approver = APPROVER.load(deps.storage)?;

    // Check sender is the approver or the parent DAO
    if approver != info.sender && dao != info.sender {
        return Err(PreProposeError::Unauthorized {});
    }

    let addr = deps.api.addr_validate(&address)?;
    pre_propose_base
        .proposal_submitted_hooks
        .add_hook(deps.storage, addr)?;

    Ok(Response::default())
}

pub fn execute_remove_approver_hook(
    deps: DepsMut,
    info: MessageInfo,
    address: String,
) -> Result<Response, PreProposeError> {
    let pre_propose_base = PrePropose::default();

    let dao = pre_propose_base.dao.load(deps.storage)?;
    let approver = APPROVER.load(deps.storage)?;

    // Check sender is the approver or the parent DAO
    if approver != info.sender && dao != info.sender {
        return Err(PreProposeError::Unauthorized {});
    }

    // Validate address
    let addr = deps.api.addr_validate(&address)?;

    // remove hook
    pre_propose_base
        .proposal_submitted_hooks
        .remove_hook(deps.storage, addr)?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::QueryExtension { msg } => match msg {
            QueryExt::Approver {} => to_json_binary(&APPROVER.load(deps.storage)?),
            QueryExt::IsPending { id } => {
                let pending = PENDING_PROPOSALS.may_load(deps.storage, id)?.is_some();
                // Force load completed proposal if not pending, throwing error
                // if not found.
                if !pending {
                    COMPLETED_PROPOSALS.load(deps.storage, id)?;
                }

                to_json_binary(&pending)
            }
            QueryExt::Proposal { id } => {
                if let Some(pending) = PENDING_PROPOSALS.may_load(deps.storage, id)? {
                    to_json_binary(&pending)
                } else {
                    // Force load completed proposal if not pending, throwing
                    // error if not found.
                    to_json_binary(&COMPLETED_PROPOSALS.load(deps.storage, id)?)
                }
            }
            QueryExt::PendingProposal { id } => {
                to_json_binary(&PENDING_PROPOSALS.load(deps.storage, id)?)
            }
            QueryExt::PendingProposals { start_after, limit } => {
                to_json_binary(&paginate_map_values(
                    deps,
                    &PENDING_PROPOSALS,
                    start_after,
                    limit,
                    Order::Ascending,
                )?)
            }
            QueryExt::ReversePendingProposals {
                start_before,
                limit,
            } => to_json_binary(&paginate_map_values(
                deps,
                &PENDING_PROPOSALS,
                start_before,
                limit,
                Order::Descending,
            )?),
            QueryExt::CompletedProposal { id } => {
                to_json_binary(&COMPLETED_PROPOSALS.load(deps.storage, id)?)
            }
            QueryExt::CompletedProposals { start_after, limit } => {
                to_json_binary(&paginate_map_values(
                    deps,
                    &COMPLETED_PROPOSALS,
                    start_after,
                    limit,
                    Order::Ascending,
                )?)
            }
            QueryExt::ReverseCompletedProposals {
                start_before,
                limit,
            } => to_json_binary(&paginate_map_values(
                deps,
                &COMPLETED_PROPOSALS,
                start_before,
                limit,
                Order::Descending,
            )?),
            QueryExt::CompletedProposalIdForCreatedProposalId { id } => {
                to_json_binary(&CREATED_PROPOSAL_TO_COMPLETED_PROPOSAL.may_load(deps.storage, id)?)
            }
        },
        _ => PrePropose::default().query(deps, env, msg),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(mut deps: DepsMut, _env: Env, msg: MigrateMsg) -> Result<Response, PreProposeError> {
    let res: Result<Response, PreProposeError> =
        PrePropose::default().migrate(deps.branch(), msg.clone());
    match msg {
        MigrateMsg::FromUnderV250 { .. } => {
            // the default migrate function above ensures >= v2.4.1 and < v2.5.0

            #[cw_serde]
            struct ProposalV241 {
                /// The status of a completed proposal.
                pub status: ProposalStatusV241,
                /// The approval ID used to identify this pending proposal.
                pub approval_id: u64,
                /// The address that created the proposal.
                pub proposer: Addr,
                /// The propose message that ought to be executed on the
                /// proposal message if this proposal is approved.
                pub msg: SingleChoiceProposeMsgV241,
                /// Snapshot of the deposit info at the time of proposal
                /// submission.
                pub deposit: Option<CheckedDepositInfoV241>,
            }

            #[cw_serde]
            enum ProposalStatusV241 {
                /// The proposal is pending approval.
                Pending {},
                /// The proposal has been approved.
                Approved {
                    /// The created proposal ID.
                    created_proposal_id: u64,
                },
                /// The proposal has been rejected.
                Rejected {},
            }

            #[cw_serde]
            struct SingleChoiceProposeMsgV241 {
                /// The title of the proposal.
                pub title: String,
                /// A description of the proposal.
                pub description: String,
                /// The messages that should be executed in response to this
                /// proposal passing.
                pub msgs: Vec<CosmosMsg<Empty>>,
                /// The address creating the proposal. If no pre-propose
                /// module is attached to this module this must always be None
                /// as the proposer is the sender of the propose message. If a
                /// pre-propose module is attached, this must be Some and will
                /// set the proposer of the proposal it creates.
                pub proposer: Option<String>,
                /// An optional vote cast by the proposer.
                pub vote: Option<SingleChoiceAutoVoteV241>,
            }

            #[cw_serde]
            #[derive(Copy)]
            #[repr(u8)]
            enum VoteV241 {
                /// Marks support for the proposal.
                Yes,
                /// Marks opposition to the proposal.
                No,
                /// Marks participation but does not count towards the ratio of
                /// support / opposed.
                Abstain,
            }

            #[cw_serde]
            struct SingleChoiceAutoVoteV241 {
                /// The proposer's position on the proposal.
                pub vote: VoteV241,
                /// An optional rationale for why this vote was cast. This can
                /// be updated, set, or removed later by the address casting
                /// the vote.
                pub rationale: Option<String>,
            }

            #[cw_serde]
            enum DepositRefundPolicyV241 {
                /// Deposits should always be refunded.
                Always,
                /// Deposits should only be refunded for passed proposals.
                OnlyPassed,
                /// Deposits should never be refunded.
                Never,
            }

            /// Counterpart to the `DepositInfo` struct which has been
            /// processed. This type should never be constructed literally and
            /// should always by built by calling `into_checked` on a
            /// `DepositInfo` instance.
            #[cw_serde]
            struct CheckedDepositInfoV241 {
                /// The address of the cw20 token to be used for proposal
                /// deposits.
                pub denom: CheckedDenomV241,
                /// The number of tokens that must be deposited to create a
                /// proposal. This is validated to be non-zero if this struct is
                /// constructed by converted via the `into_checked` method on
                /// `DepositInfo`.
                pub amount: Uint128,
                /// The policy used for refunding proposal deposits.
                pub refund_policy: DepositRefundPolicyV241,
            }

            #[cw_serde]
            enum CheckedDenomV241 {
                /// A native (bank module) asset.
                Native(String),
                /// A cw20 asset.
                Cw20(Addr),
            }

            let pending_proposals_v241: Map<u64, ProposalV241> = Map::new("pending_proposals");
            let completed_proposals_v241: Map<u64, ProposalV241> = Map::new("completed_proposals");

            // migrate proposals to add approver

            let approver = APPROVER.load(deps.storage)?;

            let pending_proposals = pending_proposals_v241
                .range(deps.storage, None, None, Order::Ascending)
                .collect::<StdResult<Vec<_>>>()?;
            for (id, proposal) in pending_proposals {
                PENDING_PROPOSALS.save(
                    deps.storage,
                    id,
                    &Proposal {
                        status: ApprovalProposalStatus::Pending {},
                        approval_id: proposal.approval_id,
                        approver: approver.clone(),
                        proposer: proposal.proposer,
                        msg: ProposeMsg {
                            title: proposal.msg.title,
                            description: proposal.msg.description,
                            msgs: proposal.msg.msgs,
                            proposer: proposal.msg.proposer,
                            vote: proposal.msg.vote.map(|vote| SingleChoiceAutoVote {
                                vote: match vote.vote {
                                    VoteV241::Yes => Vote::Yes,
                                    VoteV241::No => Vote::No,
                                    VoteV241::Abstain => Vote::Abstain,
                                },
                                rationale: vote.rationale,
                            }),
                        },
                        deposit: proposal.deposit.map(|deposit| CheckedDepositInfo {
                            denom: match deposit.denom {
                                CheckedDenomV241::Native(denom) => CheckedDenom::Native(denom),
                                CheckedDenomV241::Cw20(addr) => CheckedDenom::Cw20(addr),
                            },
                            amount: deposit.amount,
                            refund_policy: match deposit.refund_policy {
                                DepositRefundPolicyV241::Always => DepositRefundPolicy::Always,
                                DepositRefundPolicyV241::OnlyPassed => {
                                    DepositRefundPolicy::OnlyPassed
                                }
                                DepositRefundPolicyV241::Never => DepositRefundPolicy::Never,
                            },
                        }),
                    },
                )?;
            }

            let completed_proposals = completed_proposals_v241
                .range(deps.storage, None, None, Order::Ascending)
                .collect::<StdResult<Vec<_>>>()?;
            for (id, proposal) in completed_proposals {
                COMPLETED_PROPOSALS.save(
                    deps.storage,
                    id,
                    &Proposal {
                        status: match proposal.status {
                            ProposalStatusV241::Approved {
                                created_proposal_id,
                            } => ApprovalProposalStatus::Approved {
                                created_proposal_id,
                            },
                            ProposalStatusV241::Rejected {} => ApprovalProposalStatus::Rejected {},
                            // should not be possible since these are completed
                            // proposals only
                            ProposalStatusV241::Pending {} => {
                                return Err(PreProposeError::Std(StdError::generic_err(
                                    "unexpected proposal status",
                                )))
                            }
                        },
                        approval_id: proposal.approval_id,
                        approver: approver.clone(),
                        proposer: proposal.proposer,
                        msg: ProposeMsg {
                            title: proposal.msg.title,
                            description: proposal.msg.description,
                            msgs: proposal.msg.msgs,
                            proposer: proposal.msg.proposer,
                            vote: proposal.msg.vote.map(|vote| SingleChoiceAutoVote {
                                vote: match vote.vote {
                                    VoteV241::Yes => Vote::Yes,
                                    VoteV241::No => Vote::No,
                                    VoteV241::Abstain => Vote::Abstain,
                                },
                                rationale: vote.rationale,
                            }),
                        },
                        deposit: proposal.deposit.map(|deposit| CheckedDepositInfo {
                            denom: match deposit.denom {
                                CheckedDenomV241::Native(denom) => CheckedDenom::Native(denom),
                                CheckedDenomV241::Cw20(addr) => CheckedDenom::Cw20(addr),
                            },
                            amount: deposit.amount,
                            refund_policy: match deposit.refund_policy {
                                DepositRefundPolicyV241::Always => DepositRefundPolicy::Always,
                                DepositRefundPolicyV241::OnlyPassed => {
                                    DepositRefundPolicy::OnlyPassed
                                }
                                DepositRefundPolicyV241::Never => DepositRefundPolicy::Never,
                            },
                        }),
                    },
                )?;
            }
        }
        _ => {
            return Err(PreProposeError::Std(StdError::generic_err(
                "not implemented",
            )))
        }
    }
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    res
}
