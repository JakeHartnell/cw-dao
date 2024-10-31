use cosmwasm_std::{coins, Addr, Coin, CosmosMsg, Uint128};
use cw_multi_test::{App, BankSudo, Executor};

use cw_denom::CheckedDenom;
use dao_pre_propose_single as cppbps;
use dao_testing::contracts::cw20_base_contract;
use dao_voting::{
    deposit::CheckedDepositInfo,
    pre_propose::ProposalCreationPolicy,
    proposal::SingleChoiceProposeMsg as ProposeMsg,
    voting::{SingleChoiceAutoVote, Vote},
};

use crate::{
    msg::{ExecuteMsg, QueryMsg},
    query::ProposalResponse,
    testing::queries::{query_creation_policy, query_next_proposal_id},
};
use dao_proposal_single::ContractError;

use super::{queries::query_pre_proposal_single_config, CREATOR_ADDR};

// Creates a proposal then checks that the proposal was created with
// the specified messages and returns the ID of the proposal.
//
// This expects that the proposer already has the needed tokens to pay
// the deposit.
pub(crate) fn make_proposal(
    app: &mut App,
    proposal_single: &Addr,
    proposer: &str,
    msgs: Vec<CosmosMsg>,
    vote: Option<SingleChoiceAutoVote>,
) -> u64 {
    let proposal_creation_policy = query_creation_policy(app, proposal_single);

    // Collect the funding.
    let funds = match proposal_creation_policy {
        ProposalCreationPolicy::Anyone {} => vec![],
        ProposalCreationPolicy::Module {
            addr: ref pre_propose,
        } => {
            let deposit_config = query_pre_proposal_single_config(app, pre_propose);
            match deposit_config.deposit_info {
                Some(CheckedDepositInfo {
                    denom,
                    amount,
                    refund_policy: _,
                }) => match denom {
                    CheckedDenom::Native(denom) => coins(amount.u128(), denom),
                    CheckedDenom::Cw20(addr) => {
                        // Give an allowance, no funds.
                        app.execute_contract(
                            Addr::unchecked(proposer),
                            addr,
                            &cw20::Cw20ExecuteMsg::IncreaseAllowance {
                                spender: pre_propose.to_string(),
                                amount,
                                expires: None,
                            },
                            &[],
                        )
                        .unwrap();
                        vec![]
                    }
                },
                None => vec![],
            }
        }
    };

    // Make the proposal.
    match proposal_creation_policy {
        ProposalCreationPolicy::Anyone {} => app
            .execute_contract(
                Addr::unchecked(proposer),
                proposal_single.clone(),
                &ExecuteMsg::Propose(ProposeMsg {
                    title: "title".to_string(),
                    description: "description".to_string(),
                    msgs: msgs.clone(),
                    proposer: None,
                    vote,
                }),
                &[],
            )
            .unwrap(),
        ProposalCreationPolicy::Module { addr } => app
            .execute_contract(
                Addr::unchecked(proposer),
                addr,
                &cppbps::ExecuteMsg::Propose {
                    msg: cppbps::ProposeMessage::Propose {
                        title: "title".to_string(),
                        description: "description".to_string(),
                        msgs: msgs.clone(),
                        vote,
                    },
                },
                &funds,
            )
            .unwrap(),
    };
    let id = query_next_proposal_id(app, proposal_single);
    let id = id - 1;

    // Check that the proposal was created as expected.
    let proposal: ProposalResponse = app
        .wrap()
        .query_wasm_smart(proposal_single, &QueryMsg::Proposal { proposal_id: id })
        .unwrap();

    assert_eq!(proposal.proposal.proposer, Addr::unchecked(proposer));
    assert_eq!(proposal.proposal.title, "title".to_string());
    assert_eq!(proposal.proposal.description, "description".to_string());
    assert_eq!(proposal.proposal.msgs, msgs);

    id
}

pub(crate) fn vote_on_proposal(
    app: &mut App,
    proposal_single: &Addr,
    sender: &str,
    proposal_id: u64,
    vote: Vote,
) {
    app.execute_contract(
        Addr::unchecked(sender),
        proposal_single.clone(),
        &ExecuteMsg::Vote {
            proposal_id,
            vote,
            rationale: None,
        },
        &[],
    )
    .unwrap();
}

pub(crate) fn vote_on_proposal_should_fail(
    app: &mut App,
    proposal_single: &Addr,
    sender: &str,
    proposal_id: u64,
    vote: Vote,
) -> ContractError {
    app.execute_contract(
        Addr::unchecked(sender),
        proposal_single.clone(),
        &ExecuteMsg::Vote {
            proposal_id,
            vote,
            rationale: None,
        },
        &[],
    )
    .unwrap_err()
    .downcast()
    .unwrap()
}

pub(crate) fn execute_proposal_should_fail(
    app: &mut App,
    proposal_single: &Addr,
    sender: &str,
    proposal_id: u64,
) -> ContractError {
    app.execute_contract(
        Addr::unchecked(sender),
        proposal_single.clone(),
        &ExecuteMsg::Execute { proposal_id },
        &[],
    )
    .unwrap_err()
    .downcast()
    .unwrap()
}

pub(crate) fn vote_on_proposal_with_rationale(
    app: &mut App,
    proposal_single: &Addr,
    sender: &str,
    proposal_id: u64,
    vote: Vote,
    rationale: Option<String>,
) {
    app.execute_contract(
        Addr::unchecked(sender),
        proposal_single.clone(),
        &ExecuteMsg::Vote {
            proposal_id,
            vote,
            rationale,
        },
        &[],
    )
    .unwrap();
}

pub(crate) fn update_rationale(
    app: &mut App,
    proposal_single: &Addr,
    sender: &str,
    proposal_id: u64,
    rationale: Option<String>,
) {
    app.execute_contract(
        Addr::unchecked(sender),
        proposal_single.clone(),
        &ExecuteMsg::UpdateRationale {
            proposal_id,
            rationale,
        },
        &[],
    )
    .unwrap();
}

pub(crate) fn execute_proposal(
    app: &mut App,
    proposal_single: &Addr,
    sender: &str,
    proposal_id: u64,
) {
    app.execute_contract(
        Addr::unchecked(sender),
        proposal_single.clone(),
        &ExecuteMsg::Execute { proposal_id },
        &[],
    )
    .unwrap();
}

pub(crate) fn close_proposal_should_fail(
    app: &mut App,
    proposal_single: &Addr,
    sender: &str,
    proposal_id: u64,
) -> ContractError {
    app.execute_contract(
        Addr::unchecked(sender),
        proposal_single.clone(),
        &ExecuteMsg::Close { proposal_id },
        &[],
    )
    .unwrap_err()
    .downcast()
    .unwrap()
}

pub(crate) fn close_proposal(
    app: &mut App,
    proposal_single: &Addr,
    sender: &str,
    proposal_id: u64,
) {
    app.execute_contract(
        Addr::unchecked(sender),
        proposal_single.clone(),
        &ExecuteMsg::Close { proposal_id },
        &[],
    )
    .unwrap();
}

pub(crate) fn mint_natives(app: &mut App, receiver: &str, amount: Vec<Coin>) {
    app.sudo(cw_multi_test::SudoMsg::Bank(BankSudo::Mint {
        to_address: receiver.to_string(),
        amount,
    }))
    .unwrap();
}

pub(crate) fn mint_cw20s(
    app: &mut App,
    cw20_contract: &Addr,
    sender: &Addr,
    receiver: &str,
    amount: u128,
) {
    app.execute_contract(
        sender.clone(),
        cw20_contract.clone(),
        &cw20::Cw20ExecuteMsg::Mint {
            recipient: receiver.to_string(),
            amount: Uint128::new(amount),
        },
        &[],
    )
    .unwrap();
}

pub(crate) fn instantiate_cw20_base_default(app: &mut App) -> Addr {
    let cw20_id = app.store_code(cw20_base_contract());
    let cw20_instantiate = cw20_base::msg::InstantiateMsg {
        name: "cw20 token".to_string(),
        symbol: "cwtwenty".to_string(),
        decimals: 6,
        initial_balances: vec![cw20::Cw20Coin {
            address: CREATOR_ADDR.to_string(),
            amount: Uint128::new(10_000_000),
        }],
        mint: None,
        marketing: None,
    };
    app.instantiate_contract(
        cw20_id,
        Addr::unchecked("ekez"),
        &cw20_instantiate,
        &[],
        "cw20-base",
        None,
    )
    .unwrap()
}

pub(crate) fn add_proposal_hook(
    app: &mut App,
    proposal_module: &Addr,
    sender: &str,
    hook_addr: &str,
) {
    app.execute_contract(
        Addr::unchecked(sender),
        proposal_module.clone(),
        &ExecuteMsg::AddProposalHook {
            address: hook_addr.to_string(),
        },
        &[],
    )
    .unwrap();
}

pub(crate) fn add_proposal_hook_should_fail(
    app: &mut App,
    proposal_module: &Addr,
    sender: &str,
    hook_addr: &str,
) -> ContractError {
    app.execute_contract(
        Addr::unchecked(sender),
        proposal_module.clone(),
        &ExecuteMsg::AddProposalHook {
            address: hook_addr.to_string(),
        },
        &[],
    )
    .unwrap_err()
    .downcast()
    .unwrap()
}

pub(crate) fn remove_proposal_hook(
    app: &mut App,
    proposal_module: &Addr,
    sender: &str,
    hook_addr: &str,
) {
    app.execute_contract(
        Addr::unchecked(sender),
        proposal_module.clone(),
        &ExecuteMsg::RemoveProposalHook {
            address: hook_addr.to_string(),
        },
        &[],
    )
    .unwrap();
}

pub(crate) fn remove_proposal_hook_should_fail(
    app: &mut App,
    proposal_module: &Addr,
    sender: &str,
    hook_addr: &str,
) -> ContractError {
    app.execute_contract(
        Addr::unchecked(sender),
        proposal_module.clone(),
        &ExecuteMsg::RemoveProposalHook {
            address: hook_addr.to_string(),
        },
        &[],
    )
    .unwrap_err()
    .downcast()
    .unwrap()
}

pub(crate) fn add_vote_hook(app: &mut App, proposal_module: &Addr, sender: &str, hook_addr: &str) {
    app.execute_contract(
        Addr::unchecked(sender),
        proposal_module.clone(),
        &ExecuteMsg::AddVoteHook {
            address: hook_addr.to_string(),
        },
        &[],
    )
    .unwrap();
}

pub(crate) fn add_vote_hook_should_fail(
    app: &mut App,
    proposal_module: &Addr,
    sender: &str,
    hook_addr: &str,
) -> ContractError {
    app.execute_contract(
        Addr::unchecked(sender),
        proposal_module.clone(),
        &ExecuteMsg::AddVoteHook {
            address: hook_addr.to_string(),
        },
        &[],
    )
    .unwrap_err()
    .downcast()
    .unwrap()
}

pub(crate) fn remove_vote_hook(
    app: &mut App,
    proposal_module: &Addr,
    sender: &str,
    hook_addr: &str,
) {
    app.execute_contract(
        Addr::unchecked(sender),
        proposal_module.clone(),
        &ExecuteMsg::RemoveVoteHook {
            address: hook_addr.to_string(),
        },
        &[],
    )
    .unwrap();
}

pub(crate) fn remove_vote_hook_should_fail(
    app: &mut App,
    proposal_module: &Addr,
    sender: &str,
    hook_addr: &str,
) -> ContractError {
    app.execute_contract(
        Addr::unchecked(sender),
        proposal_module.clone(),
        &ExecuteMsg::RemoveVoteHook {
            address: hook_addr.to_string(),
        },
        &[],
    )
    .unwrap_err()
    .downcast()
    .unwrap()
}
