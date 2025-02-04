use cosmwasm_std::{Addr, Deps, StdResult, Uint128};

use crate::{types::SingleProposalData, ContractError};

use super::query_helpers::{
    v1_expiration_to_v2, v1_status_to_v2, v1_threshold_to_v2, v1_votes_to_v2,
};

pub fn query_proposal_count_v1(deps: Deps, proposals_addrs: Vec<Addr>) -> StdResult<Vec<u64>> {
    proposals_addrs
        .into_iter()
        .map(|proposal_addr| {
            deps.querier.query_wasm_smart(
                proposal_addr,
                &cw_proposal_single_v1::msg::QueryMsg::ProposalCount {},
            )
        })
        .collect()
}

pub fn query_proposal_count_v2(deps: Deps, proposals_addrs: Vec<Addr>) -> StdResult<Vec<u64>> {
    proposals_addrs
        .into_iter()
        .map(|proposal_addr| {
            deps.querier.query_wasm_smart(
                proposal_addr,
                &dao_proposal_single::msg::QueryMsg::ProposalCount {},
            )
        })
        .collect()
}

pub fn query_proposal_v1(
    deps: Deps,
    proposals_addrs: Vec<Addr>,
) -> Result<
    (
        Vec<dao_proposal_single::proposal::SingleChoiceProposal>,
        SingleProposalData,
    ),
    ContractError,
> {
    let mut sample_proposal = None;

    let proposals = proposals_addrs
        .into_iter()
        .map(|proposal_addr| {
            let proposals: cw_proposal_single_v1::query::ProposalListResponse =
                deps.querier.query_wasm_smart(
                    proposal_addr.clone(),
                    &cw_proposal_single_v1::msg::QueryMsg::ReverseProposals {
                        start_before: None,
                        limit: None,
                    },
                )?;

            // If we don't have a proposal in the module, we can't do tests, so bail out.
            let proposal = if proposals.proposals.is_empty() {
                Err(ContractError::NoProposalsOnModule {
                    module_addr: proposal_addr.to_string(),
                })
            } else {
                Ok(proposals.proposals[0].clone().proposal)
            }?;

            if sample_proposal.is_none() {
                sample_proposal = Some(SingleProposalData {
                    proposer: proposal.proposer.clone(),
                    start_height: proposal.start_height,
                });
            }

            Ok(dao_proposal_single::proposal::SingleChoiceProposal {
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
            })
        })
        .collect::<Result<Vec<dao_proposal_single::proposal::SingleChoiceProposal>, ContractError>>(
        )?;

    Ok((proposals, sample_proposal.unwrap()))
}

pub fn query_proposal_v2(
    deps: Deps,
    proposals_addrs: Vec<Addr>,
) -> Result<
    (
        Vec<dao_proposal_single::proposal::SingleChoiceProposal>,
        SingleProposalData,
    ),
    ContractError,
> {
    let mut sample_proposal = None;

    let proposals = proposals_addrs
        .into_iter()
        .map(|proposal_addr| {
            let proposals: dao_proposal_single::query::ProposalListResponse =
                deps.querier.query_wasm_smart(
                    proposal_addr.clone(),
                    &dao_proposal_single::msg::QueryMsg::ReverseProposals {
                        start_before: None,
                        limit: None,
                    },
                )?;

            let proposal = if proposals.proposals.is_empty() {
                Err(ContractError::NoProposalsOnModule {
                    module_addr: proposal_addr.to_string(),
                })
            } else {
                Ok(proposals.proposals[0].clone().proposal)
            }?;

            if sample_proposal.is_none() {
                sample_proposal = Some(SingleProposalData {
                    proposer: proposal.proposer.clone(),
                    start_height: proposal.start_height,
                });
            }

            Ok(proposal)
        })
        .collect::<Result<Vec<dao_proposal_single::proposal::SingleChoiceProposal>, ContractError>>(
        )?;

    Ok((proposals, sample_proposal.unwrap()))
}

pub fn query_total_voting_power_v1(
    deps: Deps,
    voting_addr: Addr,
    height: u64,
) -> StdResult<Uint128> {
    let res: cw_core_interface_v1::voting::TotalPowerAtHeightResponse =
        deps.querier.query_wasm_smart(
            voting_addr,
            &cw20_staked_balance_voting_v1::msg::QueryMsg::TotalPowerAtHeight {
                height: Some(height),
            },
        )?;
    Ok(res.power)
}

pub fn query_total_voting_power_v2(
    deps: Deps,
    voting_addr: Addr,
    height: u64,
) -> StdResult<Uint128> {
    let res: dao_interface::voting::TotalPowerAtHeightResponse = deps.querier.query_wasm_smart(
        voting_addr,
        &dao_voting_cw20_staked::msg::QueryMsg::TotalPowerAtHeight {
            height: Some(height),
        },
    )?;
    Ok(res.power)
}

pub fn query_single_voting_power_v1(
    deps: Deps,
    voting_addr: Addr,
    address: Addr,
    height: u64,
) -> StdResult<Uint128> {
    let res: cw_core_interface_v1::voting::VotingPowerAtHeightResponse =
        deps.querier.query_wasm_smart(
            voting_addr,
            &cw20_staked_balance_voting_v1::msg::QueryMsg::VotingPowerAtHeight {
                address: address.into(),
                height: Some(height),
            },
        )?;
    Ok(res.power)
}

pub fn query_single_voting_power_v2(
    deps: Deps,
    voting_addr: Addr,
    address: Addr,
    height: u64,
) -> StdResult<Uint128> {
    let res: dao_interface::voting::VotingPowerAtHeightResponse = deps.querier.query_wasm_smart(
        voting_addr,
        &dao_voting_cw20_staked::msg::QueryMsg::VotingPowerAtHeight {
            address: address.into(),
            height: Some(height),
        },
    )?;
    Ok(res.power)
}
