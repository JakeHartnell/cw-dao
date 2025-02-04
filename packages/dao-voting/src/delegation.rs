use cosmwasm_schema::{
    cw_serde,
    serde::{de::DeserializeOwned, Serialize},
    QueryResponses,
};
use cosmwasm_std::{Addr, Decimal, DepsMut, StdResult, Uint128};
use cw_storage_plus::Map;
use dao_interface::voting::InfoResponse;

use crate::{proposal::Ballot, voting::VotingPowerWithDelegation};

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Returns contract version info
    #[returns(InfoResponse)]
    Info {},
    /// Returns registration info for a delegate, optionally at a given height.
    #[returns(RegistrationResponse)]
    Registration {
        delegate: String,
        height: Option<u64>,
    },
    /// Returns the paginated list of active delegates.
    #[returns(DelegatesResponse)]
    Delegates {
        start_after: Option<String>,
        limit: Option<u32>,
    },
    /// Returns the delegations by a delegator, optionally at a given height.
    /// Uses the current block height if not provided.
    #[returns(DelegationsResponse)]
    Delegations {
        delegator: String,
        height: Option<u64>,
        offset: Option<u64>,
        limit: Option<u64>,
    },
    /// Returns the VP delegated to a delegate that has not yet been used in
    /// votes cast by delegators in a specific proposal. This updates
    /// immediately via vote hooks (instead of being delayed 1 block like other
    /// historical queries), making it safe to vote multiple times in the same
    /// block. Proposal modules are responsible for maintaining the effective VP
    /// cap when a delegator overrides a delegate's vote.
    #[returns(UnvotedDelegatedVotingPowerResponse)]
    UnvotedDelegatedVotingPower {
        delegate: String,
        proposal_module: String,
        proposal_id: u64,
        height: u64,
    },
    /// Returns the proposal modules synced from the DAO.
    #[returns(Vec<Addr>)]
    ProposalModules {
        start_after: Option<String>,
        limit: Option<u32>,
    },
    /// Returns the voting power hook callers.
    #[returns(Vec<Addr>)]
    VotingPowerHookCallers {
        start_after: Option<String>,
        limit: Option<u32>,
    },
    /// Returns the config.
    #[returns(Config)]
    Config {},
}

#[cw_serde]
pub struct RegistrationResponse {
    /// Whether or not the delegate is registered.
    pub registered: bool,
    /// The total voting power delegated to the delegate. If not registered,
    /// this may still be nonzero if the delegate was registered in the past.
    pub power: Uint128,
    /// The height at which registration was checked.
    pub height: u64,
}

#[cw_serde]
pub struct DelegatesResponse {
    /// The delegates.
    pub delegates: Vec<DelegateResponse>,
}

#[cw_serde]
pub struct DelegateResponse {
    /// The delegate.
    pub delegate: Addr,
    /// The total voting power delegated to the delegate.
    pub power: Uint128,
}

#[cw_serde]
#[derive(Default)]
pub struct DelegationsResponse {
    /// The delegations.
    pub delegations: Vec<DelegationResponse>,
    /// The height at which the delegations were loaded.
    pub height: u64,
}

#[cw_serde]
pub struct DelegationResponse {
    /// the delegate that can vote on behalf of the delegator.
    pub delegate: Addr,
    /// the percent of the delegator's voting power that is delegated to the
    /// delegate.
    pub percent: Decimal,
    /// whether or not the delegation is active (i.e. the delegate is still
    /// registered at the corresponding block). this can only be false if the
    /// delegate was registered when the delegation was created and isn't
    /// anymore.
    pub active: bool,
}

#[cw_serde]
#[derive(Default)]
pub struct UnvotedDelegatedVotingPowerResponse {
    /// The total unvoted delegated voting power.
    pub total: Uint128,
    /// The unvoted delegated voting power in effect, with configured
    /// constraints applied, such as the VP cap.
    pub effective: Uint128,
}

#[cw_serde]
pub struct Delegate {}

#[cw_serde]
pub struct Delegation {
    /// the delegate that can vote on behalf of the delegator.
    pub delegate: Addr,
    /// the percent of the delegator's voting power that is delegated to the
    /// delegate.
    pub percent: Decimal,
}

#[cw_serde]
pub struct Config {
    /// the number of blocks a delegation is valid for, after which it must be
    /// renewed by the delegator. if not set, the delegation will never expire.
    pub delegation_validity_blocks: Option<u64>,
    /// the total number of delegations a member can have. this should be set
    /// based on the max gas allowed in a single block for the given chain.
    ///
    /// this limit is relevant for two reasons:
    ///  1. when voting power is updated for a delegator, we must loop through
    ///     all of their delegates and update their delegated voting power
    ///  2. when a delegator casts a vote on a proposal that overrides their
    ///     delegates' votes, we must loop through all of their delegates and
    ///     update the proposal vote tally accordingly
    ///
    /// in tests on Neutron, with a block max gas of 30M (which is one of the
    /// lowest gas limits on any chain), we found that 50 delegations is a safe
    /// upper bound.
    pub max_delegations: u64,
}

/// Calculate delegated voting power given a member's total voting power and a
/// percent delegated.
pub fn calculate_delegated_vp(vp: Uint128, percent: Decimal) -> Uint128 {
    if percent.is_zero() || vp.is_zero() {
        return Uint128::zero();
    }

    vp.mul_floor(percent)
}

// DELEGATE VOTE OVERRIDE: if this is the first time this member voted, override
// their delegates' votes with the delegator's vote.
//
// subtract the delegator's VP from the vote tally of all of their delegates who
// already voted on this proposal, in order to override their vote with the
// delegator's preference.
//
// we must load all delegations and update each. if this partially fails, the
// vote tallies will be incorrect, so the entire vote transaction should fail.
// we need to prevent this from happening by limiting the number of delegations
// a member can have in order to ensure votes can always be cast.
#[allow(clippy::too_many_arguments)]
pub fn handle_delegate_vote_override<Vote: Serialize + DeserializeOwned>(
    deps: DepsMut,
    delegator: &Addr,
    delegation_module: &Option<Addr>,
    proposal_module: &Addr,
    proposal_id: u64,
    proposal_start_height: u64,
    vote_power: &VotingPowerWithDelegation,
    ballots: Map<(u64, &Addr), Ballot<Vote>>,
    remove_vote: &mut impl FnMut(&Vote, Uint128) -> StdResult<()>,
) -> StdResult<()> {
    if let Some(delegation_module) = delegation_module {
        let delegations = deps
            .querier
            .query_wasm_smart::<DelegationsResponse>(
                delegation_module,
                &QueryMsg::Delegations {
                    delegator: delegator.to_string(),
                    height: Some(proposal_start_height),
                    offset: None,
                    limit: None,
                },
                // ensure query error gets returned if it fails.
            )?
            .delegations;

        for DelegationResponse {
            delegate,
            percent,
            active,
        } in delegations
        {
            // if delegation is not active, skip.
            if !active {
                continue;
            }

            // if delegate voted already, untally the VP the delegator delegated
            // to them since the delegate's vote is being overridden.
            if let Some(mut delegate_ballot) =
                ballots.may_load(deps.storage, (proposal_id, &delegate))?
            {
                // get the delegate's current unvoted delegated VP. since we are
                // currently overriding this delegate's vote, this UDVP response
                // will not yet take into account the loss of this current
                // voter's delegated VP, so we have to do math below to remove
                // this voter's VP from the delegate's effective VP. the vote
                // hook at the end of the proposal module's vote function will
                // update this UDVP in the delegation module for future votes.
                //
                // NOTE: this UDVP query reflects updates immediately, instead
                // of waiting 1 block to take effect like other historical
                // queries, so this will reflect the updated UDVP from the vote
                // hooks within the same block, making it safe to vote twice in
                // the same block.
                let prev_udvp: UnvotedDelegatedVotingPowerResponse =
                    deps.querier.query_wasm_smart(
                        delegation_module,
                        &QueryMsg::UnvotedDelegatedVotingPower {
                            delegate: delegate.to_string(),
                            proposal_module: proposal_module.to_string(),
                            proposal_id,
                            height: proposal_start_height,
                        },
                    )?;

                let voter_delegated_vp = calculate_delegated_vp(vote_power.individual, percent);

                // subtract this voter's delegated VP from the delegate's total
                // VP, and cap the result at the delegate's effective VP, to
                // ensure we properly take into account the configured VP cap.
                // if the delegate has been delegated in total more than this
                // voter's delegated VP above the cap, they will not lose any
                // VP. they will lose part or all of this voter's delegated VP
                // based on how their total VP ranks relative to the configured
                // cap.
                let new_effective_delegated = prev_udvp
                    .total
                    .checked_sub(voter_delegated_vp)?
                    .min(prev_udvp.effective);

                // if the new effective VP is less than the previous effective
                // VP, update the delegate's ballot and tally.
                if new_effective_delegated < prev_udvp.effective {
                    // how much VP the delegate is losing based on this voter's
                    // VP and the cap.
                    let diff = prev_udvp.effective - new_effective_delegated;

                    // update ballot total and vote tally by removing the lost
                    // delegated VP only. this makes sure to fully preserve the
                    // delegate's personal VP even if they lose all delegated VP
                    // due to delegators overriding votes.
                    delegate_ballot.power -= diff;
                    remove_vote(&delegate_ballot.vote, diff)?;

                    ballots.save(deps.storage, (proposal_id, &delegate), &delegate_ballot)?;
                }
            }
        }
    }

    Ok(())
}
