#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_json_binary, Binary, CosmosMsg, Deps, DepsMut, Empty, Env, MessageInfo, Response, StdResult,
    WasmMsg,
};
use cw2::set_contract_version;

use dao_interface::state::ModuleInstantiateCallback;
use dao_pre_propose_base::{
    error::PreProposeError, msg::ExecuteMsg as PreProposeExecuteBase, state::PreProposeContract,
};
use dao_voting::approval::{ApprovalExecuteExt, ApproverProposeMessage};
use dao_voting::pre_propose::PreProposeSubmissionPolicy;
use dao_voting::status::Status;

use crate::msg::{
    BaseInstantiateMsg, ExecuteExt, ExecuteMsg, InstantiateMsg, MigrateMsg, ProposeMessageInternal,
    QueryExt, QueryMsg,
};
use crate::state::{
    PRE_PROPOSE_APPROVAL_CONTRACT, PRE_PROPOSE_ID_TO_PROPOSAL_ID, PROPOSAL_ID_TO_PRE_PROPOSE_ID,
};

pub(crate) const CONTRACT_NAME: &str = "crates.io:dao-pre-propose-approver";
pub(crate) const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

type PrePropose = PreProposeContract<Empty, ExecuteExt, QueryExt, Empty, ApproverProposeMessage>;
type PreProposeApprovalExecuteMsg =
    PreProposeExecuteBase<ApproverProposeMessage, ApprovalExecuteExt>;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, PreProposeError> {
    // This contract does not handle deposits or allow submission permissions
    // since only the approval-* contract can create proposals. Just hardcode
    // the pre-propose-base instantiate message.
    let base_instantiate_msg = BaseInstantiateMsg {
        deposit_info: None,
        submission_policy: PreProposeSubmissionPolicy::Specific {
            dao_members: true,
            allowlist: vec![],
            denylist: vec![],
        },
        extension: Empty {},
    };
    // Default pre-propose-base instantiation
    let resp = PrePropose::default().instantiate(
        deps.branch(),
        env.clone(),
        info,
        base_instantiate_msg,
    )?;
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // Validate and save the address of the pre-propose-approval-* contract
    let addr = deps.api.addr_validate(&msg.pre_propose_approval_contract)?;
    PRE_PROPOSE_APPROVAL_CONTRACT.save(deps.storage, &addr)?;

    Ok(resp.set_data(to_json_binary(&ModuleInstantiateCallback {
        msgs: vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: addr.to_string(),
                msg: to_json_binary(&PreProposeApprovalExecuteMsg::AddProposalSubmittedHook {
                    address: env.contract.address.to_string(),
                })?,
                funds: vec![],
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: addr.to_string(),
                msg: to_json_binary(&PreProposeApprovalExecuteMsg::Extension {
                    msg: ApprovalExecuteExt::UpdateApprover {
                        address: env.contract.address.to_string(),
                    },
                })?,
                funds: vec![],
            }),
        ],
    })?))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, PreProposeError> {
    match msg {
        // Override default pre-propose-base behavior
        ExecuteMsg::Propose { msg } => execute_propose(deps, info, msg),
        ExecuteMsg::ProposalCompletedHook {
            proposal_id,
            new_status,
        } => execute_proposal_completed(deps, info, proposal_id, new_status),
        ExecuteMsg::Extension { msg } => match msg {
            ExecuteExt::ResetApprover {} => execute_reset_approver(deps, env, info),
        },
        // Override config updates since they don't apply.
        ExecuteMsg::UpdateConfig { .. } => Err(PreProposeError::Unsupported {}),
        ExecuteMsg::UpdateSubmissionPolicy { .. } => Err(PreProposeError::Unsupported {}),
        _ => PrePropose::default().execute(deps, env, info, msg),
    }
}

pub fn execute_propose(
    deps: DepsMut,
    info: MessageInfo,
    msg: ApproverProposeMessage,
) -> Result<Response, PreProposeError> {
    // Check that this is coming from the expected approval contract
    let approval_contract = PRE_PROPOSE_APPROVAL_CONTRACT.load(deps.storage)?;
    if info.sender != approval_contract {
        return Err(PreProposeError::Unauthorized {});
    }

    // Get pre_propose_id, transform proposal for the approver
    // Here we make sure that there are no messages that can be executed
    let (pre_propose_id, sanitized_msg) = match msg {
        ApproverProposeMessage::Propose {
            title,
            description,
            approval_id: pre_propose_id,
        } => (
            pre_propose_id,
            ProposeMessageInternal::Propose {
                title,
                description,
                msgs: vec![],
                proposer: Some(info.sender.to_string()),
            },
        ),
    };

    let proposal_module = PrePropose::default().proposal_module.load(deps.storage)?;
    let proposal_id = deps.querier.query_wasm_smart(
        &proposal_module,
        &dao_interface::proposal::Query::NextProposalId {},
    )?;
    PROPOSAL_ID_TO_PRE_PROPOSE_ID.save(deps.storage, proposal_id, &pre_propose_id)?;
    PRE_PROPOSE_ID_TO_PROPOSAL_ID.save(deps.storage, pre_propose_id, &proposal_id)?;

    let propose_messsage = WasmMsg::Execute {
        contract_addr: proposal_module.into_string(),
        msg: to_json_binary(&sanitized_msg)?,
        funds: vec![],
    };
    Ok(Response::default().add_message(propose_messsage))
}

pub fn execute_proposal_completed(
    deps: DepsMut,
    info: MessageInfo,
    proposal_id: u64,
    new_status: Status,
) -> Result<Response, PreProposeError> {
    // Safety check, this message can only come from the proposal module
    let proposal_module = PrePropose::default().proposal_module.load(deps.storage)?;
    if info.sender != proposal_module {
        return Err(PreProposeError::NotModule {});
    }

    // Get approval pre-propose id
    let pre_propose_id = PROPOSAL_ID_TO_PRE_PROPOSE_ID.load(deps.storage, proposal_id)?;

    // Get approval contract address
    let approval_contract = PRE_PROPOSE_APPROVAL_CONTRACT.load(deps.storage)?;

    // On completion send rejection or approval message
    let msg = match new_status {
        Status::Closed => Some(WasmMsg::Execute {
            contract_addr: approval_contract.into_string(),
            msg: to_json_binary(&PreProposeApprovalExecuteMsg::Extension {
                msg: ApprovalExecuteExt::Reject { id: pre_propose_id },
            })?,
            funds: vec![],
        }),
        Status::Executed => Some(WasmMsg::Execute {
            contract_addr: approval_contract.into_string(),
            msg: to_json_binary(&PreProposeApprovalExecuteMsg::Extension {
                msg: ApprovalExecuteExt::Approve { id: pre_propose_id },
            })?,
            funds: vec![],
        }),
        _ => None,
    };

    // If Status is not Executed or Closed, throw error
    match msg {
        Some(msg) => Ok(Response::default()
            .add_message(msg)
            .add_attribute("method", "execute_proposal_completed_hook")
            .add_attribute("proposal", proposal_id.to_string())),
        None => Err(PreProposeError::NotCompleted { status: new_status }),
    }
}

pub fn execute_reset_approver(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, PreProposeError> {
    // Check that this is coming from the DAO.
    let dao = PrePropose::default().dao.load(deps.storage)?;
    if info.sender != dao {
        return Err(PreProposeError::Unauthorized {});
    }

    let pre_propose_approval_contract = PRE_PROPOSE_APPROVAL_CONTRACT.load(deps.storage)?;

    let reset_messages = vec![
        // Remove the proposal submitted hook.
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: pre_propose_approval_contract.to_string(),
            msg: to_json_binary(&PreProposeApprovalExecuteMsg::RemoveProposalSubmittedHook {
                address: env.contract.address.to_string(),
            })?,
            funds: vec![],
        }),
        // Set pre-propose-approval approver to the DAO.
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: pre_propose_approval_contract.to_string(),
            msg: to_json_binary(&PreProposeApprovalExecuteMsg::Extension {
                msg: ApprovalExecuteExt::UpdateApprover {
                    address: dao.to_string(),
                },
            })?,
            funds: vec![],
        }),
    ];

    Ok(Response::default().add_messages(reset_messages))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::CanPropose { address } => {
            let approval_contract = PRE_PROPOSE_APPROVAL_CONTRACT.load(deps.storage)?;
            let can_propose = address == approval_contract;
            to_json_binary(&can_propose)
        }
        QueryMsg::QueryExtension { msg } => match msg {
            QueryExt::PreProposeApprovalContract {} => {
                to_json_binary(&PRE_PROPOSE_APPROVAL_CONTRACT.load(deps.storage)?)
            }
            QueryExt::PreProposeApprovalIdForApproverProposalId { id } => {
                to_json_binary(&PROPOSAL_ID_TO_PRE_PROPOSE_ID.may_load(deps.storage, id)?)
            }
            QueryExt::ApproverProposalIdForPreProposeApprovalId { id } => {
                to_json_binary(&PRE_PROPOSE_ID_TO_PROPOSAL_ID.may_load(deps.storage, id)?)
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
