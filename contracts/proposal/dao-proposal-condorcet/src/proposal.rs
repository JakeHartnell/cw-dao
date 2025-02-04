use cosmwasm_schema::cw_serde;
use cosmwasm_std::{to_json_binary, Addr, BlockInfo, StdResult, SubMsg, Uint128, WasmMsg};
use cw_utils::Expiration;
use dao_voting::{
    reply::mask_proposal_execution_proposal_id, threshold::PercentageThreshold,
    voting::does_vote_count_pass,
};

use crate::{
    config::Config,
    msg::Choice,
    tally::{Tally, Winner},
};

#[cw_serde]
pub struct Proposal {
    last_status: Status,

    pub proposer: Addr,

    pub quorum: PercentageThreshold,
    pub min_voting_period: Option<Expiration>,

    pub close_on_execution_failure: bool,
    pub total_power: Uint128,

    pub id: u32,
    pub choices: Vec<Choice>,
}

#[cw_serde]
#[derive(Copy)]
pub enum Status {
    /// The proposal is open for voting.
    Open,
    /// The proposal has been rejected.
    Rejected,
    /// The proposal has passed.
    Passed { winner: u32 },
    /// The proposal has been passed and executed.
    Executed,
    /// The proposal has failed or expired and has been closed. A
    /// proposal deposit refund has been issued if applicable.
    Closed,
    /// The proposal's execution failed.
    ExecutionFailed,
}

#[cw_serde]
pub struct ProposalResponse {
    pub proposal: Proposal,
    pub tally: Tally,
}

fn status(block: &BlockInfo, proposal: &Proposal, tally: &Tally) -> Status {
    match proposal.last_status {
        Status::Rejected
        | Status::Passed { .. }
        | Status::Executed
        | Status::Closed
        | Status::ExecutionFailed => proposal.last_status,
        Status::Open => {
            if proposal
                .min_voting_period
                .is_some_and(|min| !min.is_expired(block))
            {
                return Status::Open;
            }

            let winner = tally.winner;
            let expired = tally.expiration.is_expired(block);
            let quorum = does_vote_count_pass(
                proposal.total_power - tally.power_outstanding,
                proposal.total_power,
                proposal.quorum,
            );

            if expired && !quorum {
                Status::Rejected
            } else {
                match winner {
                    Winner::Never => Status::Rejected,
                    Winner::None => {
                        if expired {
                            Status::Rejected
                        } else {
                            Status::Open
                        }
                    }
                    Winner::Some(winner) => {
                        if expired && quorum {
                            Status::Passed { winner }
                        } else {
                            Status::Open
                        }
                    }
                    Winner::Undisputed(winner) => {
                        if quorum {
                            Status::Passed { winner }
                        } else {
                            Status::Open
                        }
                    }
                }
            }
        }
    }
}

impl Proposal {
    pub(crate) fn new(
        block: &BlockInfo,
        config: &Config,
        proposer: Addr,
        id: u32,
        choices: Vec<Choice>,
        total_power: Uint128,
    ) -> Self {
        Self {
            last_status: Status::Open,

            min_voting_period: config.min_voting_period.map(|m| m.after(block)),
            quorum: config.quorum,
            close_on_execution_failure: config.close_proposals_on_execution_failure,

            id,
            proposer,
            choices,
            total_power,
        }
    }

    pub(crate) fn update_status(&mut self, block: &BlockInfo, tally: &Tally) -> Status {
        self.last_status = status(block, self, tally);
        self.last_status
    }

    pub fn status(&self, block: &BlockInfo, tally: &Tally) -> Status {
        status(block, self, tally)
    }

    // To test that status is updated before responding to queries.
    #[cfg(test)]
    pub fn last_status(&self) -> Status {
        self.last_status
    }

    pub(crate) fn set_closed(&mut self) {
        debug_assert_eq!(self.last_status, Status::Rejected);

        self.last_status = Status::Closed;
    }

    /// Sets the proposal's status to executed and returns a
    /// submessage to be executed.
    pub(crate) fn set_executed(&mut self, dao: Addr, winner: u32) -> StdResult<SubMsg> {
        debug_assert_eq!(self.last_status, Status::Passed { winner });

        self.last_status = Status::Executed;

        let msgs = self.choices[winner as usize].msgs.clone();
        let core_exec = WasmMsg::Execute {
            contract_addr: dao.into_string(),
            msg: to_json_binary(&dao_interface::msg::ExecuteMsg::ExecuteProposalHook { msgs })?,
            funds: vec![],
        };
        Ok(if self.close_on_execution_failure {
            let masked_id = mask_proposal_execution_proposal_id(self.id as u64);
            SubMsg::reply_on_error(core_exec, masked_id)
        } else {
            SubMsg::new(core_exec)
        })
    }

    pub(crate) fn set_execution_failed(&mut self) {
        debug_assert_eq!(self.last_status, Status::Executed);

        self.last_status = Status::ExecutionFailed;
    }
}
