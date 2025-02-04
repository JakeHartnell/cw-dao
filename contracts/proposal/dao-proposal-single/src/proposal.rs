use std::ops::Add;

use crate::query::ProposalResponse;
use crate::state::PROPOSAL_COUNT;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, BlockInfo, CosmosMsg, Decimal, Empty, StdResult, Storage, Uint128};
use cw_utils::Expiration;
use dao_voting::status::Status;
use dao_voting::threshold::{PercentageThreshold, Threshold};
use dao_voting::veto::VetoConfig;
use dao_voting::voting::{does_vote_count_fail, does_vote_count_pass, Votes};

#[cw_serde]
pub struct SingleChoiceProposal {
    /// The title of the proposal
    pub title: String,
    /// The main body of the proposal text
    pub description: String,
    /// The address that created this proposal.
    pub proposer: Addr,
    /// The block height at which this proposal was created. Voting
    /// power queries should query for voting power at this block
    /// height.
    pub start_height: u64,
    /// The minimum amount of time this proposal must remain open for
    /// voting. The proposal may not pass unless this is expired or
    /// None.
    pub min_voting_period: Option<Expiration>,
    /// The the time at which this proposal will expire and close for
    /// additional votes.
    pub expiration: Expiration,
    /// The threshold at which this proposal will pass.
    pub threshold: Threshold,
    /// The total amount of voting power at the time of this
    /// proposal's creation.
    pub total_power: Uint128,
    /// The messages that will be executed should this proposal pass.
    pub msgs: Vec<CosmosMsg<Empty>>,
    /// The proposal status
    pub status: Status,
    /// The vote tally, including voting power delegated to delegates. This
    /// tally changes in any of the following situations:
    /// - any voter casts a new vote
    /// - revoting is enabled and any voter changes their vote
    /// - a delegator overrides their delegate's vote
    pub votes: Votes,
    /// The vote tally, excluding voting power delegated to delegates. This
    /// tally changes in any of the following situations:
    /// - any voter casts a new vote
    /// - revoting is enabled and any voter changes their vote
    ///
    /// This field is used to determine if a proposal has completed early
    /// (before its expiration) when revoting is disabled. Delegated voting
    /// power is not counted, since a delegator can override their delegate's
    /// vote and affect the outcome. Only individual voting power of delegators
    /// and delegates is counted. This field is irrelevant when revoting is
    /// enabled and will not be checked.
    ///
    /// This field should match the `votes` field when delegation is not in use.
    /// When using delegation, this field will be equal to `votes` if no
    /// delegate with delegations has voted, or all delegators have overridden
    /// their delegates' votes; it will be smaller than `votes` if any delegate
    /// has voted on behalf of a delegator that has not cast their own vote.
    pub individual_votes: Votes,
    /// Whether or not revoting is enabled. If revoting is enabled, a proposal
    /// cannot pass until the voting period has elapsed.
    pub allow_revoting: bool,
    /// Optional veto configuration. If set to `None`, veto option
    /// is disabled. Otherwise contains the configuration for veto flow.
    pub veto: Option<VetoConfig>,
    /// The address of the delegation module associated with this proposal (if
    /// one existed when the proposal was created).
    pub delegation_module: Option<Addr>,
}

pub fn next_proposal_id(store: &dyn Storage) -> StdResult<u64> {
    Ok(PROPOSAL_COUNT.may_load(store)?.unwrap_or_default() + 1)
}

pub fn advance_proposal_id(store: &mut dyn Storage) -> StdResult<u64> {
    let id: u64 = next_proposal_id(store)?;
    PROPOSAL_COUNT.save(store, &id)?;
    Ok(id)
}

impl SingleChoiceProposal {
    /// Consumes the proposal and returns a version which may be used
    /// in a query response. Why is this necessary? Proposal
    /// statuses are only updated on vote, execute, and close
    /// events; thus, it is possible that, if the proposal expires since
    /// a vote has occurred, the status we read from the proposal status
    /// may be out of date. This method recomputes the status so that
    /// queries get accurate information.
    pub fn into_response(mut self, block: &BlockInfo, id: u64) -> StdResult<ProposalResponse> {
        self.update_status(block)?;
        Ok(ProposalResponse { id, proposal: self })
    }

    /// Gets the current status of the proposal.
    pub fn current_status(&self, block: &BlockInfo) -> StdResult<Status> {
        match self.status {
            Status::Open if self.is_passed(block) => match &self.veto {
                // if prop is passed and veto is configured, calculate timelock
                // expiration. if it's expired, this proposal has passed.
                // otherwise, set status to `VetoTimelock`.
                Some(veto_config) => {
                    let expiration = self.expiration.add(veto_config.timelock_duration)?;

                    if expiration.is_expired(block) {
                        Ok(Status::Passed)
                    } else {
                        Ok(Status::VetoTimelock { expiration })
                    }
                }
                // Otherwise the proposal is simply passed
                None => Ok(Status::Passed),
            },
            Status::Open if self.expiration.is_expired(block) || self.is_rejected(block) => {
                Ok(Status::Rejected)
            }
            Status::VetoTimelock { expiration } => {
                // if prop timelock expired, proposal is now passed.
                if expiration.is_expired(block) {
                    Ok(Status::Passed)
                } else {
                    Ok(self.status)
                }
            }
            _ => Ok(self.status),
        }
    }

    /// Sets a proposals status to its current status.
    pub fn update_status(&mut self, block: &BlockInfo) -> StdResult<()> {
        let new_status = self.current_status(block)?;
        self.status = new_status;
        Ok(())
    }

    /// Returns true iff this proposal is sure to pass (even before expiration
    /// if no future sequence of possible votes can cause it to fail).
    ///
    /// Before expiration, this ignores voting power cast via delegation, since
    /// delegators can override their delegates' votes and change the outcome.
    /// Once expired, this considers all voting power, including delegated,
    /// since nothing can change the outcome.
    pub fn is_passed(&self, block: &BlockInfo) -> bool {
        let is_expired = self.expiration.is_expired(block);

        // If re-voting is allowed nothing is known until the proposal
        // has expired.
        if self.allow_revoting && !is_expired {
            return false;
        }
        // If the min voting period is set and not expired the
        // proposal can not yet be passed. This gives DAO members some
        // time to remove liquidity / scheme on a recovery plan if a
        // single actor accumulates enough tokens to unilaterally pass
        // proposals.
        if let Some(min) = self.min_voting_period {
            if !min.is_expired(block) {
                return false;
            }
        }

        let votes_to_consider = if is_expired {
            &self.votes
        } else {
            &self.individual_votes
        };

        match self.threshold {
            Threshold::AbsolutePercentage { percentage } => {
                let options = self.total_power - votes_to_consider.abstain;
                does_vote_count_pass(votes_to_consider.yes, options, percentage)
            }
            Threshold::ThresholdQuorum { threshold, quorum } => {
                if !does_vote_count_pass(votes_to_consider.total(), self.total_power, quorum) {
                    return false;
                }

                if self.expiration.is_expired(block) {
                    // If the quorum is met and the proposal is
                    // expired the number of votes needed to pass a
                    // proposal is compared to the number of votes on
                    // the proposal.
                    let options = votes_to_consider.total() - votes_to_consider.abstain;
                    does_vote_count_pass(votes_to_consider.yes, options, threshold)
                } else {
                    let options = self.total_power - votes_to_consider.abstain;
                    does_vote_count_pass(votes_to_consider.yes, options, threshold)
                }
            }
            Threshold::AbsoluteCount { threshold } => votes_to_consider.yes >= threshold,
        }
    }

    /// As above for the passed check, used to check if a proposal is already
    /// rejected. This ignores voting power cast via delegation, since
    /// delegators can override their delegates votes. This math should only
    /// consider individual votes cast.
    pub fn is_rejected(&self, block: &BlockInfo) -> bool {
        let is_expired = self.expiration.is_expired(block);

        // If re-voting is allowed and the proposal is not expired no
        // information is known.
        if self.allow_revoting && !is_expired {
            return false;
        }

        let votes_to_consider = if is_expired {
            &self.votes
        } else {
            &self.individual_votes
        };

        match self.threshold {
            Threshold::AbsolutePercentage {
                percentage: percentage_needed,
            } => {
                let options = self.total_power - votes_to_consider.abstain;

                // If there is a 100% passing threshold..
                if percentage_needed == PercentageThreshold::Percent(Decimal::percent(100)) {
                    if options == Uint128::zero() {
                        // and there are no possible votes (zero
                        // voting power or all abstain), then this
                        // proposal has been rejected.
                        return true;
                    } else {
                        // and there are possible votes, then this is
                        // rejected if there is a single no vote.
                        //
                        // We need this check becuase otherwise when
                        // we invert the threshold (`Decimal::one() -
                        // threshold`) we get a 0% requirement for no
                        // votes. Zero no votes do indeed meet a 0%
                        // threshold.
                        return votes_to_consider.no >= Uint128::new(1);
                    }
                }

                does_vote_count_fail(votes_to_consider.no, options, percentage_needed)
            }
            Threshold::ThresholdQuorum { threshold, quorum } => {
                match (
                    does_vote_count_pass(votes_to_consider.total(), self.total_power, quorum),
                    self.expiration.is_expired(block),
                ) {
                    // Has met quorum and is expired.
                    (true, true) => {
                        // => consider only votes cast and see if no
                        //    votes meet threshold.
                        let options = votes_to_consider.total() - votes_to_consider.abstain;

                        // If there is a 100% passing threshold..
                        if threshold == PercentageThreshold::Percent(Decimal::percent(100)) {
                            if options == Uint128::zero() {
                                // and there are no possible votes (zero
                                // voting power or all abstain), then this
                                // proposal has been rejected.
                                return true;
                            } else {
                                // and there are possible votes, then this is
                                // rejected if there is a single no vote.
                                //
                                // We need this check becuase
                                // otherwise when we invert the
                                // threshold (`Decimal::one() -
                                // threshold`) we get a 0% requirement
                                // for no votes. Zero no votes do
                                // indeed meet a 0% threshold.
                                return votes_to_consider.no >= Uint128::new(1);
                            }
                        }
                        does_vote_count_fail(votes_to_consider.no, options, threshold)
                    }
                    // Has met quorum and is not expired.
                    // | Hasn't met quorum and is not expired.
                    (true, false) | (false, false) => {
                        // => consider all possible votes and see if
                        //    no votes meet threshold.
                        let options = self.total_power - votes_to_consider.abstain;

                        // If there is a 100% passing threshold..
                        if threshold == PercentageThreshold::Percent(Decimal::percent(100)) {
                            if options == Uint128::zero() {
                                // and there are no possible votes (zero
                                // voting power or all abstain), then this
                                // proposal has been rejected.
                                return true;
                            } else {
                                // and there are possible votes, then this is
                                // rejected if there is a single no vote.
                                //
                                // We need this check because otherwise
                                // when we invert the threshold
                                // (`Decimal::one() - threshold`) we
                                // get a 0% requirement for no
                                // votes. Zero no votes do indeed meet
                                // a 0% threshold.
                                return votes_to_consider.no >= Uint128::new(1);
                            }
                        }

                        does_vote_count_fail(votes_to_consider.no, options, threshold)
                    }
                    // Hasn't met quorum requirement and voting has closed => rejected.
                    (false, true) => true,
                }
            }
            Threshold::AbsoluteCount { threshold } => {
                // If all the outstanding votes voting yes would not
                // cause this proposal to pass then it is rejected.
                let outstanding_votes = self.total_power - votes_to_consider.total();
                votes_to_consider.yes + outstanding_votes < threshold
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use cosmwasm_std::{
        testing::{mock_dependencies, mock_env},
        Decimal,
    };

    fn setup_prop(
        threshold: Threshold,
        votes: Votes,
        total_power: Uint128,
        is_expired: bool,
        min_voting_period_elapsed: bool,
        allow_revoting: bool,
    ) -> (SingleChoiceProposal, BlockInfo) {
        let block = mock_env().block;
        let expiration = match is_expired {
            true => Expiration::AtHeight(block.height - 5),
            false => Expiration::AtHeight(block.height + 100),
        };
        let min_voting_period = match min_voting_period_elapsed {
            true => Expiration::AtHeight(block.height - 5),
            false => Expiration::AtHeight(block.height + 5),
        };

        let prop = SingleChoiceProposal {
            title: "Demo".to_string(),
            description: "Info".to_string(),
            proposer: Addr::unchecked("test"),
            start_height: 100,
            expiration,
            min_voting_period: Some(min_voting_period),
            allow_revoting,
            msgs: vec![],
            status: Status::Open,
            threshold,
            veto: None,
            total_power,
            votes: votes.clone(),
            individual_votes: votes,
            delegation_module: None,
        };
        (prop, block)
    }

    fn check_is_passed(
        threshold: Threshold,
        votes: Votes,
        total_power: Uint128,
        is_expired: bool,
        min_voting_period_elapsed: bool,
        allow_revoting: bool,
    ) -> bool {
        let (prop, block) = setup_prop(
            threshold,
            votes,
            total_power,
            is_expired,
            min_voting_period_elapsed,
            allow_revoting,
        );
        prop.is_passed(&block)
    }

    fn check_is_rejected(
        threshold: Threshold,
        votes: Votes,
        total_power: Uint128,
        is_expired: bool,
        min_voting_period_elapsed: bool,
        allow_revoting: bool,
    ) -> bool {
        let (prop, block) = setup_prop(
            threshold,
            votes,
            total_power,
            is_expired,
            min_voting_period_elapsed,
            allow_revoting,
        );
        prop.is_rejected(&block)
    }

    #[test]
    fn test_pass_majority_percentage() {
        let threshold = Threshold::AbsolutePercentage {
            percentage: PercentageThreshold::Majority {},
        };
        let votes = Votes {
            yes: Uint128::new(7),
            no: Uint128::new(4),
            abstain: Uint128::new(2),
        };

        // 15 total votes. 7 yes and 2 abstain. Majority threshold. This
        // should pass.
        assert!(check_is_passed(
            threshold.clone(),
            votes.clone(),
            Uint128::new(15),
            false,
            true,
            false,
        ));
        // Proposal being expired should not effect those results.
        assert!(check_is_passed(
            threshold.clone(),
            votes.clone(),
            Uint128::new(15),
            true,
            true,
            false
        ));

        // More votes == higher threshold => not passed.
        assert!(!check_is_passed(
            threshold,
            votes,
            Uint128::new(17),
            false,
            true,
            false
        ));
    }

    #[test]
    fn test_min_voting_period_no_early_pass() {
        let threshold = Threshold::AbsolutePercentage {
            percentage: PercentageThreshold::Majority {},
        };
        let votes = Votes {
            yes: Uint128::new(7),
            no: Uint128::new(4),
            abstain: Uint128::new(2),
        };

        // Does not pass if min voting period is not expired.
        assert!(!check_is_passed(
            threshold.clone(),
            votes.clone(),
            Uint128::new(15),
            false,
            false,
            false,
        ));
        // Should not be rejected either.
        assert!(!check_is_rejected(
            threshold.clone(),
            votes.clone(),
            Uint128::new(15),
            false,
            false,
            false,
        ));

        // Min voting period being expired makes this pass.
        assert!(check_is_passed(
            threshold,
            votes,
            Uint128::new(15),
            false,
            true,
            false
        ));
    }

    #[test]
    fn test_min_voting_period_early_rejection() {
        let threshold = Threshold::AbsolutePercentage {
            percentage: PercentageThreshold::Majority {},
        };
        let votes = Votes {
            yes: Uint128::new(4),
            no: Uint128::new(7),
            abstain: Uint128::new(2),
        };

        // Proposal has not passed.
        assert!(!check_is_passed(
            threshold.clone(),
            votes.clone(),
            Uint128::new(15),
            false,
            false,
            false,
        ));
        // Should be rejected despite the min voting period not being
        // passed.
        assert!(check_is_rejected(
            threshold,
            votes,
            Uint128::new(15),
            false,
            false,
            false,
        ));
    }

    #[test]
    fn test_revoting_majority_no_pass() {
        // Revoting being allowed means that proposals may not be
        // passed or rejected before they expire.
        let threshold = Threshold::AbsolutePercentage {
            percentage: PercentageThreshold::Majority {},
        };
        let votes = Votes {
            yes: Uint128::new(7),
            no: Uint128::new(4),
            abstain: Uint128::new(2),
        };

        // 15 total votes. 7 yes and 2 abstain. Majority threshold. This
        // should pass but revoting is enabled.
        assert!(!check_is_passed(
            threshold.clone(),
            votes.clone(),
            Uint128::new(15),
            false,
            true,
            true,
        ));
        // Proposal being expired should cause the proposal to be
        // passed as votes may no longer be cast.
        assert!(check_is_passed(
            threshold,
            votes,
            Uint128::new(15),
            true,
            true,
            true
        ));
    }

    #[test]
    fn test_revoting_majority_rejection() {
        // Revoting being allowed means that proposals may not be
        // passed or rejected before they expire.
        let threshold = Threshold::AbsolutePercentage {
            percentage: PercentageThreshold::Majority {},
        };
        let votes = Votes {
            yes: Uint128::new(4),
            no: Uint128::new(7),
            abstain: Uint128::new(2),
        };

        // Not expired, revoting allowed => no rejection.
        assert!(!check_is_rejected(
            threshold.clone(),
            votes.clone(),
            Uint128::new(15),
            false,
            true,
            true
        ));

        // Expired, revoting allowed => rejection.
        assert!(check_is_rejected(
            threshold,
            votes,
            Uint128::new(15),
            true,
            true,
            true
        ));
    }

    /// Simple checks for absolute count passing and failing
    /// conditions.
    #[test]
    fn test_absolute_count_threshold() {
        let threshold = Threshold::AbsoluteCount {
            threshold: Uint128::new(10),
        };

        assert!(check_is_passed(
            threshold.clone(),
            Votes {
                yes: Uint128::new(10),
                no: Uint128::zero(),
                abstain: Uint128::zero(),
            },
            Uint128::new(100),
            false,
            true,
            false
        ));

        assert!(check_is_rejected(
            threshold.clone(),
            Votes {
                yes: Uint128::new(9),
                no: Uint128::new(1),
                abstain: Uint128::zero()
            },
            Uint128::new(10),
            false,
            true,
            false
        ));

        assert!(!check_is_rejected(
            threshold.clone(),
            Votes {
                yes: Uint128::new(9),
                no: Uint128::new(1),
                abstain: Uint128::zero()
            },
            Uint128::new(11),
            false,
            true,
            false
        ));

        assert!(!check_is_passed(
            threshold,
            Votes {
                yes: Uint128::new(9),
                no: Uint128::new(1),
                abstain: Uint128::zero()
            },
            Uint128::new(11),
            false,
            true,
            false
        ));
    }

    /// Tests that revoting works as expected with an absolute count
    /// style threshold.
    #[test]
    fn test_absolute_count_threshold_revoting() {
        let threshold = Threshold::AbsoluteCount {
            threshold: Uint128::new(10),
        };

        assert!(!check_is_passed(
            threshold.clone(),
            Votes {
                yes: Uint128::new(10),
                no: Uint128::zero(),
                abstain: Uint128::zero(),
            },
            Uint128::new(100),
            false,
            true,
            true
        ));
        assert!(check_is_passed(
            threshold.clone(),
            Votes {
                yes: Uint128::new(10),
                no: Uint128::zero(),
                abstain: Uint128::zero(),
            },
            Uint128::new(100),
            true,
            true,
            true
        ));

        assert!(!check_is_rejected(
            threshold.clone(),
            Votes {
                yes: Uint128::new(9),
                no: Uint128::new(1),
                abstain: Uint128::zero()
            },
            Uint128::new(10),
            false,
            true,
            true
        ));
        assert!(check_is_rejected(
            threshold,
            Votes {
                yes: Uint128::new(9),
                no: Uint128::new(1),
                abstain: Uint128::zero()
            },
            Uint128::new(10),
            true,
            true,
            true
        ));
    }

    #[test]
    fn test_tricky_pass() {
        let threshold = Threshold::AbsolutePercentage {
            percentage: PercentageThreshold::Percent(Decimal::from_ratio(7u32, 13u32)),
        };
        let votes = Votes {
            yes: Uint128::new(7),
            no: Uint128::new(6),
            abstain: Uint128::zero(),
        };
        assert!(check_is_passed(
            threshold,
            votes,
            Uint128::new(13),
            false,
            true,
            false
        ))
    }

    #[test]
    fn test_weird_failure_rounding() {
        let threshold = Threshold::AbsolutePercentage {
            percentage: PercentageThreshold::Percent(Decimal::from_ratio(6u32, 13u32)),
        };
        let votes = Votes {
            yes: Uint128::new(6),
            no: Uint128::new(7),
            abstain: Uint128::zero(),
        };
        assert!(check_is_passed(
            threshold.clone(),
            votes.clone(),
            Uint128::new(13),
            false,
            true,
            false
        ));
        assert!(!check_is_rejected(
            threshold,
            votes,
            Uint128::new(13),
            false,
            true,
            false
        ));
    }

    #[test]
    fn test_tricky_pass_majority() {
        let threshold = Threshold::AbsolutePercentage {
            percentage: PercentageThreshold::Majority {},
        };
        let votes = Votes {
            yes: Uint128::new(7),
            no: Uint128::new(6),
            abstain: Uint128::zero(),
        };
        assert!(check_is_passed(
            threshold.clone(),
            votes.clone(),
            Uint128::new(13),
            false,
            true,
            false
        ));
        assert!(!check_is_passed(
            threshold,
            votes,
            Uint128::new(14),
            false,
            true,
            false
        ))
    }

    #[test]
    fn test_reject_majority_percentage() {
        let percent = Threshold::AbsolutePercentage {
            percentage: PercentageThreshold::Majority {},
        };

        // 4 YES, 7 NO, 2 ABSTAIN
        let votes = Votes {
            yes: Uint128::new(4),
            no: Uint128::new(7),
            abstain: Uint128::new(2),
        };

        // 15 total voting power
        // 7 / (15 - 2) > 50%
        // Expiry does not matter
        assert!(check_is_rejected(
            percent.clone(),
            votes.clone(),
            Uint128::new(15),
            false,
            true,
            false,
        ));
        assert!(check_is_rejected(
            percent.clone(),
            votes.clone(),
            Uint128::new(15),
            true,
            true,
            false
        ));

        // 17 total voting power
        // 7 / (17 - 2) < 50%
        assert!(!check_is_rejected(
            percent.clone(),
            votes.clone(),
            Uint128::new(17),
            false,
            true,
            false
        ));
        assert!(!check_is_rejected(
            percent.clone(),
            votes.clone(),
            Uint128::new(17),
            true,
            true,
            false
        ));

        // Rejected if total was lower
        assert!(check_is_rejected(
            percent.clone(),
            votes.clone(),
            Uint128::new(14),
            false,
            true,
            false
        ));
        assert!(check_is_rejected(
            percent,
            votes,
            Uint128::new(14),
            true,
            true,
            false
        ));
    }

    #[test]
    fn proposal_passed_quorum() {
        let quorum = Threshold::ThresholdQuorum {
            threshold: PercentageThreshold::Percent(Decimal::percent(50)),
            quorum: PercentageThreshold::Percent(Decimal::percent(40)),
        };
        // all non-yes votes are counted for quorum
        let passing = Votes {
            yes: Uint128::new(7),
            no: Uint128::new(3),
            abstain: Uint128::new(2),
        };
        // abstain votes are not counted for threshold => yes / (yes + no + veto)
        let passes_ignoring_abstain = Votes {
            yes: Uint128::new(6),
            no: Uint128::new(6),
            abstain: Uint128::new(5),
        };
        // fails any way you look at it
        let failing = Votes {
            yes: Uint128::new(6),
            no: Uint128::new(7),
            abstain: Uint128::new(2),
        };

        // first, expired (voting period over)
        // over quorum (40% of 30 = 12), over threshold (7/12 > 50%)
        assert!(check_is_passed(
            quorum.clone(),
            passing.clone(),
            Uint128::new(30),
            true,
            true,
            false,
        ));
        // under quorum it is not passing (40% of 33 = 13.2 > 13)
        assert!(!check_is_passed(
            quorum.clone(),
            passing.clone(),
            Uint128::new(33),
            true,
            true,
            false
        ));
        // over quorum, threshold passes if we ignore abstain
        // 17 total votes w/ abstain => 40% quorum of 40 total
        // 6 yes / (6 yes + 4 no + 2 votes) => 50% threshold
        assert!(check_is_passed(
            quorum.clone(),
            passes_ignoring_abstain.clone(),
            Uint128::new(40),
            true,
            true,
            false,
        ));
        // over quorum, but under threshold fails also
        assert!(!check_is_passed(
            quorum.clone(),
            failing,
            Uint128::new(20),
            true,
            true,
            false
        ));

        // now, check with open voting period
        // would pass if closed, but fail here, as remaining votes no -> fail
        assert!(!check_is_passed(
            quorum.clone(),
            passing.clone(),
            Uint128::new(30),
            false,
            true,
            false
        ));
        assert!(!check_is_passed(
            quorum.clone(),
            passes_ignoring_abstain.clone(),
            Uint128::new(40),
            false,
            true,
            false
        ));
        // if we have threshold * total_weight as yes votes this must pass
        assert!(check_is_passed(
            quorum.clone(),
            passing.clone(),
            Uint128::new(14),
            false,
            true,
            false
        ));
        // all votes have been cast, some abstain
        assert!(check_is_passed(
            quorum.clone(),
            passes_ignoring_abstain,
            Uint128::new(17),
            false,
            true,
            false
        ));
        // 3 votes uncast, if they all vote no, we have 7 yes, 7 no+veto, 2 abstain (out of 16)
        assert!(check_is_passed(
            quorum,
            passing,
            Uint128::new(16),
            false,
            true,
            false
        ));
    }

    #[test]
    fn proposal_rejected_quorum() {
        let quorum = Threshold::ThresholdQuorum {
            threshold: PercentageThreshold::Majority {},
            quorum: PercentageThreshold::Percent(Decimal::percent(40)),
        };
        // all non-yes votes are counted for quorum
        let rejecting = Votes {
            yes: Uint128::new(3),
            no: Uint128::new(8),
            abstain: Uint128::new(2),
        };
        // abstain votes are not counted for threshold => yes / (yes + no)
        let rejected_ignoring_abstain = Votes {
            yes: Uint128::new(4),
            no: Uint128::new(8),
            abstain: Uint128::new(5),
        };
        // fails any way you look at it
        let failing = Votes {
            yes: Uint128::new(5),
            no: Uint128::new(8),
            abstain: Uint128::new(2),
        };

        // first, expired (voting period over)
        // over quorum (40% of 30 = 12), over threshold (7/11 > 50%)
        assert!(check_is_rejected(
            quorum.clone(),
            rejecting.clone(),
            Uint128::new(30),
            true,
            true,
            false
        ));
        // Total power of 33. 13 total votes. 8 no votes, 3 yes, 2
        // abstain. 39.3% turnout. Expired. As it is expired we see if
        // the 8 no votes excede the 50% failing threshold, which they
        // do.
        assert!(check_is_rejected(
            quorum.clone(),
            rejecting.clone(),
            Uint128::new(33),
            true,
            true,
            false
        ));

        // over quorum, threshold passes if we ignore abstain
        // 17 total votes w/ abstain => 40% quorum of 40 total
        // 6 no / (6 no + 4 yes + 2 votes) => 50% threshold
        assert!(check_is_rejected(
            quorum.clone(),
            rejected_ignoring_abstain.clone(),
            Uint128::new(40),
            true,
            true,
            false
        ));

        // Over quorum, but under threshold fails if the proposal is
        // not expired. If the proposal is expired though it passes as
        // the total vote count used is the number of votes, and not
        // the total number of votes avaliable.
        assert!(check_is_rejected(
            quorum.clone(),
            failing.clone(),
            Uint128::new(20),
            true,
            true,
            false
        ));
        assert!(!check_is_rejected(
            quorum.clone(),
            failing,
            Uint128::new(20),
            false,
            true,
            false
        ));

        // Voting is still open so assume rest of votes are yes
        // threshold not reached
        assert!(!check_is_rejected(
            quorum.clone(),
            rejecting.clone(),
            Uint128::new(30),
            false,
            true,
            false
        ));
        assert!(!check_is_rejected(
            quorum.clone(),
            rejected_ignoring_abstain.clone(),
            Uint128::new(40),
            false,
            true,
            false
        ));
        // if we have threshold * total_weight as no votes this must reject
        assert!(check_is_rejected(
            quorum.clone(),
            rejecting.clone(),
            Uint128::new(14),
            false,
            true,
            false
        ));
        // all votes have been cast, some abstain
        assert!(check_is_rejected(
            quorum.clone(),
            rejected_ignoring_abstain,
            Uint128::new(17),
            false,
            true,
            false
        ));
        // 3 votes uncast, if they all vote yes, we have 7 no, 7 yes+veto, 2 abstain (out of 16)
        assert!(check_is_rejected(
            quorum,
            rejecting,
            Uint128::new(16),
            false,
            true,
            false
        ));
    }

    #[test]
    fn quorum_edge_cases() {
        // When we pass absolute threshold (everyone else voting no,
        // we pass), but still don't hit quorum.
        let quorum = Threshold::ThresholdQuorum {
            threshold: PercentageThreshold::Percent(Decimal::percent(60)),
            quorum: PercentageThreshold::Percent(Decimal::percent(80)),
        };

        // Try 9 yes, 1 no (out of 15) -> 90% voter threshold, 60%
        // absolute threshold, still no quorum doesn't matter if
        // expired or not.
        let missing_voters = Votes {
            yes: Uint128::new(9),
            no: Uint128::new(1),
            abstain: Uint128::new(0),
        };
        assert!(!check_is_passed(
            quorum.clone(),
            missing_voters.clone(),
            Uint128::new(15),
            false,
            true,
            false
        ));
        assert!(!check_is_passed(
            quorum.clone(),
            missing_voters,
            Uint128::new(15),
            true,
            true,
            false
        ));

        // 1 less yes, 3 vetos and this passes only when expired.
        let wait_til_expired = Votes {
            yes: Uint128::new(8),
            no: Uint128::new(4),
            abstain: Uint128::new(0),
        };
        assert!(!check_is_passed(
            quorum.clone(),
            wait_til_expired.clone(),
            Uint128::new(15),
            false,
            true,
            false
        ));
        assert!(check_is_passed(
            quorum.clone(),
            wait_til_expired,
            Uint128::new(15),
            true,
            true,
            false
        ));

        // 9 yes and 3 nos passes early
        let passes_early = Votes {
            yes: Uint128::new(9),
            no: Uint128::new(3),
            abstain: Uint128::new(0),
        };
        assert!(check_is_passed(
            quorum.clone(),
            passes_early.clone(),
            Uint128::new(15),
            false,
            true,
            false
        ));
        assert!(check_is_passed(
            quorum,
            passes_early,
            Uint128::new(15),
            true,
            true,
            false
        ));
    }

    #[test]
    fn test_proposal_ids_advance() {
        // do they advance, lets find out!
        let storage = &mut mock_dependencies().storage;
        let next = next_proposal_id(storage).unwrap();
        assert_eq!(next, 1);

        let now = advance_proposal_id(storage).unwrap();
        assert_eq!(now, next);

        let next = next_proposal_id(storage).unwrap();
        assert_eq!(next, 2);

        let now = advance_proposal_id(storage).unwrap();
        assert_eq!(now, next);
    }
}
