use cosmwasm_std::{Addr, Uint128};
use cw_multi_test::App;

use crate::utils::query_helpers::{
    v1_expiration_to_v2, v1_status_to_v2, v1_threshold_to_v2, v1_votes_to_v2,
};

#[derive(PartialEq, Debug, Clone)]
pub struct TestState {
    pub proposal_count: u64,
    pub proposal: dao_proposal_single::proposal::SingleChoiceProposal,
    pub total_power: Uint128,
    pub single_power: Uint128,
}

pub fn query_proposal_v1(
    app: &mut App,
    proposal_addr: Addr,
) -> (u64, dao_proposal_single::proposal::SingleChoiceProposal) {
    // proposal count
    let proposal_count: u64 = app
        .wrap()
        .query_wasm_smart(
            proposal_addr.clone(),
            &cw_proposal_single_v1::msg::QueryMsg::ProposalCount {},
        )
        .unwrap();

    // query proposal
    let proposal = app
        .wrap()
        .query_wasm_smart::<cw_proposal_single_v1::query::ProposalListResponse>(
            proposal_addr,
            &cw_proposal_single_v1::msg::QueryMsg::ListProposals {
                start_after: None,
                limit: None,
            },
        )
        .unwrap()
        .proposals[0]
        .clone()
        .proposal;

    let proposal = dao_proposal_single::proposal::SingleChoiceProposal {
        title: proposal.title,
        description: proposal.description,
        proposer: proposal.proposer,
        start_height: proposal.start_height,
        min_voting_period: proposal.min_voting_period.map(v1_expiration_to_v2),
        expiration: v1_expiration_to_v2(proposal.expiration),
        threshold: v1_threshold_to_v2(proposal.threshold),
        total_power: proposal.total_power,
        msgs: proposal.msgs,
        status: v1_status_to_v2(proposal.status),
        votes: v1_votes_to_v2(proposal.votes.clone()),
        individual_votes: v1_votes_to_v2(proposal.votes),
        allow_revoting: proposal.allow_revoting,
        veto: None,
        delegation_module: None,
    };

    (proposal_count, proposal)
}

pub fn query_proposal_v2(
    app: &mut App,
    proposal_addr: Addr,
) -> (u64, dao_proposal_single::proposal::SingleChoiceProposal) {
    // proposal count
    let proposal_count: u64 = app
        .wrap()
        .query_wasm_smart(
            proposal_addr.clone(),
            &dao_proposal_single::msg::QueryMsg::ProposalCount {},
        )
        .unwrap();

    // query proposal
    let proposal = app
        .wrap()
        .query_wasm_smart::<dao_proposal_single::query::ProposalListResponse>(
            proposal_addr,
            &dao_proposal_single::msg::QueryMsg::ListProposals {
                start_after: None,
                limit: None,
            },
        )
        .unwrap()
        .proposals[0]
        .clone()
        .proposal;

    (proposal_count, proposal)
}

pub fn query_state_v1_cw20(app: &mut App, proposal_addr: Addr, voting_addr: Addr) -> TestState {
    let (proposal_count, proposal) = query_proposal_v1(app, proposal_addr);

    // query total voting power
    let total_power = app
        .wrap()
        .query_wasm_smart::<cw_core_interface_v1::voting::TotalPowerAtHeightResponse>(
            voting_addr.clone(),
            &cw20_staked_balance_voting_v1::msg::QueryMsg::TotalPowerAtHeight {
                height: Some(proposal.start_height),
            },
        )
        .unwrap()
        .power;

    // query single voting power
    let single_power = app
        .wrap()
        .query_wasm_smart::<cw_core_interface_v1::voting::VotingPowerAtHeightResponse>(
            voting_addr,
            &cw20_staked_balance_voting_v1::msg::QueryMsg::VotingPowerAtHeight {
                address: proposal.proposer.to_string(),
                height: Some(proposal.start_height),
            },
        )
        .unwrap()
        .power;

    TestState {
        proposal_count,
        proposal,
        total_power,
        single_power,
    }
}

pub fn query_state_v2_cw20(app: &mut App, proposal_addr: Addr, voting_addr: Addr) -> TestState {
    let (proposal_count, proposal) = query_proposal_v2(app, proposal_addr);

    // query total voting power
    let total_power = app
        .wrap()
        .query_wasm_smart::<dao_interface::voting::TotalPowerAtHeightResponse>(
            voting_addr.clone(),
            &dao_voting_cw20_staked::msg::QueryMsg::TotalPowerAtHeight {
                height: Some(proposal.start_height),
            },
        )
        .unwrap()
        .power;

    // query single voting power
    let single_power = app
        .wrap()
        .query_wasm_smart::<dao_interface::voting::VotingPowerAtHeightResponse>(
            voting_addr,
            &dao_voting_cw20_staked::msg::QueryMsg::VotingPowerAtHeight {
                address: proposal.proposer.to_string(),
                height: Some(proposal.start_height),
            },
        )
        .unwrap()
        .power;

    TestState {
        proposal_count,
        proposal,
        total_power,
        single_power,
    }
}

pub fn query_state_v1_cw4(app: &mut App, proposal_addr: Addr, voting_addr: Addr) -> TestState {
    let (proposal_count, proposal) = query_proposal_v1(app, proposal_addr);

    // query total voting power
    let total_power = app
        .wrap()
        .query_wasm_smart::<cw_core_interface_v1::voting::TotalPowerAtHeightResponse>(
            voting_addr.clone(),
            &cw4_voting_v1::msg::QueryMsg::TotalPowerAtHeight {
                height: Some(proposal.start_height),
            },
        )
        .unwrap()
        .power;

    // query single voting power
    let single_power = app
        .wrap()
        .query_wasm_smart::<cw_core_interface_v1::voting::VotingPowerAtHeightResponse>(
            voting_addr,
            &cw4_voting_v1::msg::QueryMsg::VotingPowerAtHeight {
                address: proposal.proposer.to_string(),
                height: Some(proposal.start_height),
            },
        )
        .unwrap()
        .power;

    TestState {
        proposal_count,
        proposal,
        total_power,
        single_power,
    }
}

pub fn query_state_v2_cw4(app: &mut App, proposal_addr: Addr, voting_addr: Addr) -> TestState {
    let (proposal_count, proposal) = query_proposal_v2(app, proposal_addr);

    // query total voting power
    let total_power = app
        .wrap()
        .query_wasm_smart::<dao_interface::voting::TotalPowerAtHeightResponse>(
            voting_addr.clone(),
            &dao_voting_cw4::msg::QueryMsg::TotalPowerAtHeight {
                height: Some(proposal.start_height),
            },
        )
        .unwrap()
        .power;

    // query single voting power
    let single_power = app
        .wrap()
        .query_wasm_smart::<dao_interface::voting::VotingPowerAtHeightResponse>(
            voting_addr,
            &dao_voting_cw4::msg::QueryMsg::VotingPowerAtHeight {
                address: proposal.proposer.to_string(),
                height: Some(proposal.start_height),
            },
        )
        .unwrap()
        .power;

    TestState {
        proposal_count,
        proposal,
        total_power,
        single_power,
    }
}
