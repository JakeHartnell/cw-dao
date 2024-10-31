use cosmwasm_std::{Addr, Uint128};
use cw_multi_test::App;
use dao_interface::state::{ProposalModule, ProposalModuleStatus};

use cw_hooks::HooksResponse;
use dao_pre_propose_single as dpps;
use dao_voting::pre_propose::ProposalCreationPolicy;

use crate::{
    msg::QueryMsg,
    query::{ProposalListResponse, ProposalResponse, VoteListResponse, VoteResponse},
    state::Config,
};

pub(crate) fn query_deposit_config_and_pre_propose_module(
    app: &App,
    proposal_single: &Addr,
) -> (dpps::Config, Addr) {
    let proposal_creation_policy = query_creation_policy(app, proposal_single);

    if let ProposalCreationPolicy::Module { addr: module_addr } = proposal_creation_policy {
        let deposit_config = query_pre_proposal_single_config(app, &module_addr);

        (deposit_config, module_addr)
    } else {
        panic!("no pre-propose module.")
    }
}

pub(crate) fn query_proposal_config(app: &App, proposal_single: &Addr) -> Config {
    app.wrap()
        .query_wasm_smart(proposal_single, &QueryMsg::Config {})
        .unwrap()
}

pub(crate) fn query_creation_policy(app: &App, proposal_single: &Addr) -> ProposalCreationPolicy {
    app.wrap()
        .query_wasm_smart(proposal_single, &QueryMsg::ProposalCreationPolicy {})
        .unwrap()
}

pub(crate) fn query_list_proposals(
    app: &App,
    proposal_single: &Addr,
    start_after: Option<u64>,
    limit: Option<u64>,
) -> ProposalListResponse {
    app.wrap()
        .query_wasm_smart(
            proposal_single,
            &QueryMsg::ListProposals { start_after, limit },
        )
        .unwrap()
}

pub(crate) fn query_list_votes(
    app: &App,
    proposal_single: &Addr,
    proposal_id: u64,
    start_after: Option<String>,
    limit: Option<u64>,
) -> VoteListResponse {
    app.wrap()
        .query_wasm_smart(
            proposal_single,
            &QueryMsg::ListVotes {
                proposal_id,
                start_after,
                limit,
            },
        )
        .unwrap()
}

pub(crate) fn query_vote(
    app: &App,
    proposal_module: &Addr,
    who: &str,
    proposal_id: u64,
) -> VoteResponse {
    app.wrap()
        .query_wasm_smart(
            proposal_module,
            &QueryMsg::GetVote {
                proposal_id,
                voter: who.to_string(),
            },
        )
        .unwrap()
}

pub(crate) fn query_proposal_hooks(app: &App, proposal_single: &Addr) -> HooksResponse {
    app.wrap()
        .query_wasm_smart(proposal_single, &QueryMsg::ProposalHooks {})
        .unwrap()
}

pub(crate) fn query_vote_hooks(app: &App, proposal_single: &Addr) -> HooksResponse {
    app.wrap()
        .query_wasm_smart(proposal_single, &QueryMsg::VoteHooks {})
        .unwrap()
}

pub(crate) fn query_list_proposals_reverse(
    app: &App,
    proposal_single: &Addr,
    start_before: Option<u64>,
    limit: Option<u64>,
) -> ProposalListResponse {
    app.wrap()
        .query_wasm_smart(
            proposal_single,
            &QueryMsg::ReverseProposals {
                start_before,
                limit,
            },
        )
        .unwrap()
}

pub(crate) fn query_pre_proposal_single_config(app: &App, pre_propose: &Addr) -> dpps::Config {
    app.wrap()
        .query_wasm_smart(pre_propose, &dpps::QueryMsg::Config {})
        .unwrap()
}

pub(crate) fn query_pre_proposal_single_deposit_info(
    app: &App,
    pre_propose: &Addr,
    proposal_id: u64,
) -> dpps::DepositInfoResponse {
    app.wrap()
        .query_wasm_smart(pre_propose, &dpps::QueryMsg::DepositInfo { proposal_id })
        .unwrap()
}

pub(crate) fn query_single_proposal_module(app: &App, core_addr: &Addr) -> Addr {
    let modules: Vec<ProposalModule> = app
        .wrap()
        .query_wasm_smart(
            core_addr,
            &dao_interface::msg::QueryMsg::ProposalModules {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();

    // Filter out disabled modules.
    let modules = modules
        .into_iter()
        .filter(|module| module.status == ProposalModuleStatus::Enabled)
        .collect::<Vec<_>>();

    assert_eq!(
        modules.len(),
        1,
        "wrong proposal module count. expected 1, got {}",
        modules.len()
    );

    modules.into_iter().next().unwrap().address
}

pub(crate) fn query_dao_token(app: &App, core_addr: &Addr) -> Addr {
    let voting_module = query_voting_module(app, core_addr);
    app.wrap()
        .query_wasm_smart(
            voting_module,
            &dao_interface::voting::Query::TokenContract {},
        )
        .unwrap()
}

pub(crate) fn query_voting_module(app: &App, core_addr: &Addr) -> Addr {
    app.wrap()
        .query_wasm_smart(core_addr, &dao_interface::msg::QueryMsg::VotingModule {})
        .unwrap()
}

pub(crate) fn query_balance_cw20<T: Into<String>, U: Into<String>>(
    app: &App,
    contract_addr: T,
    address: U,
) -> Uint128 {
    let msg = cw20::Cw20QueryMsg::Balance {
        address: address.into(),
    };
    let result: cw20::BalanceResponse = app.wrap().query_wasm_smart(contract_addr, &msg).unwrap();
    result.balance
}

pub(crate) fn query_balance_native(app: &App, who: &str, denom: &str) -> Uint128 {
    let res = app.wrap().query_balance(who, denom).unwrap();
    res.amount
}

pub(crate) fn query_proposal(app: &App, proposal_single: &Addr, id: u64) -> ProposalResponse {
    app.wrap()
        .query_wasm_smart(proposal_single, &QueryMsg::Proposal { proposal_id: id })
        .unwrap()
}

pub(crate) fn query_next_proposal_id(app: &App, proposal_single: &Addr) -> u64 {
    app.wrap()
        .query_wasm_smart(proposal_single, &QueryMsg::NextProposalId {})
        .unwrap()
}

pub(crate) fn query_proposal_count(app: &App, proposal_single: &Addr) -> u64 {
    app.wrap()
        .query_wasm_smart(proposal_single, &QueryMsg::ProposalCount {})
        .unwrap()
}
