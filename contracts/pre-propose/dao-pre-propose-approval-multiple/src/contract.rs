#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_json_binary, Binary, Deps, DepsMut, Empty, Env, MessageInfo, Order, Response, StdResult,
    SubMsg, WasmMsg,
};
use cw2::set_contract_version;
use cw_paginate_storage::paginate_map_values;
use dao_pre_propose_base::{
    error::PreProposeError, msg::ExecuteMsg as ExecuteBase, state::PreProposeContract,
};
use dao_voting::approval::{ApprovalProposalStatus, ApproverProposeMessage};
use dao_voting::deposit::DepositRefundPolicy;
use dao_voting::proposal::MultipleChoiceProposeMsg as ProposeMsg;

use crate::msg::{
    ExecuteExt, ExecuteMsg, InstantiateExt, InstantiateMsg, MigrateMsg, ProposeMessage,
    ProposeMessageInternal, QueryExt, QueryMsg,
};
use crate::state::{
    advance_approval_id, Proposal, APPROVER, COMPLETED_PROPOSALS,
    CREATED_PROPOSAL_TO_COMPLETED_PROPOSAL, PENDING_PROPOSALS,
};

pub(crate) const CONTRACT_NAME: &str = "crates.io:dao-pre-propose-approval-multiple";
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
            choices,
            vote,
        } => ProposeMsg {
            title,
            description,
            choices,
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

    // Save the proposal and its information as pending.
    PENDING_PROPOSALS.save(
        deps.storage,
        approval_id,
        &Proposal {
            status: ApprovalProposalStatus::Pending {},
            approval_id,
            proposer: info.sender,
            msg: propose_msg_internal,
            deposit: config.deposit_info,
        },
    )?;

    Ok(Response::default()
        .add_messages(deposit_messages)
        .add_submessages(hooks_msgs)
        .add_attribute("method", "pre-propose")
        .add_attribute("id", approval_id.to_string()))
}

pub fn execute_approve(
    deps: DepsMut,
    info: MessageInfo,
    id: u64,
) -> Result<Response, PreProposeError> {
    // Check sender is the approver
    let approver = APPROVER.load(deps.storage)?;
    if approver != info.sender {
        return Err(PreProposeError::Unauthorized {});
    }

    // Load proposal and send propose message to the proposal module
    let proposal = PENDING_PROPOSALS.may_load(deps.storage, id)?;
    match proposal {
        Some(proposal) => {
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
    // Check sender is the approver
    let approver = APPROVER.load(deps.storage)?;
    if approver != info.sender {
        return Err(PreProposeError::Unauthorized {});
    }

    let Proposal {
        approval_id,
        proposer,
        msg,
        deposit,
        ..
    } = PENDING_PROPOSALS
        .may_load(deps.storage, id)?
        .ok_or(PreProposeError::ProposalNotFound {})?;

    COMPLETED_PROPOSALS.save(
        deps.storage,
        id,
        &Proposal {
            status: ApprovalProposalStatus::Rejected {},
            approval_id,
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
    let res = PrePropose::default().migrate(deps.branch(), msg);
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    res
}
