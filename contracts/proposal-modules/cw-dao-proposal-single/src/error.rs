use std::u64;

use cosmwasm_std::StdError;
use cw_dao_voting::reply::error::TagError;
use cw_indexable_hooks::HookError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error(transparent)]
    Std(#[from] StdError),

    #[error(transparent)]
    HookError(#[from] HookError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error(transparent)]
    ThresholdError(#[from] cw_dao_voting::threshold::ThresholdError),

    #[error(transparent)]
    VotingError(#[from] cw_dao_voting::error::VotingError),

    #[error("Suggested proposal expiration is larger than the maximum proposal duration")]
    InvalidExpiration {},

    #[error("No such proposal ({id})")]
    NoSuchProposal { id: u64 },

    #[error("Proposal is ({size}) bytes, must be <= ({max}) bytes")]
    ProposalTooLarge { size: u64, max: u64 },

    #[error("Proposal is not open ({id})")]
    NotOpen { id: u64 },

    #[error("Proposal is expired ({id})")]
    Expired { id: u64 },

    #[error("Not registered to vote (no voting power) at time of proposal creation.")]
    NotRegistered {},

    #[error("Already voted. This proposal does not support revoting.")]
    AlreadyVoted {},

    #[error("Already cast a vote with that option. Change your vote to revote.")]
    AlreadyCast {},

    #[error("Proposal is not in 'passed' state.")]
    NotPassed {},

    #[error("Proposal has already been executed.")]
    AlreadyExecuted {},

    #[error("Proposal is closed.")]
    Closed {},

    #[error("Only rejected proposals may be closed.")]
    WrongCloseStatus {},

    #[error("The DAO is currently inactive, you cannot create proposals")]
    InactiveDao {},

    #[error("min_voting_period and max_voting_period must have the same units (height or time)")]
    DurationUnitsConflict {},

    #[error("Min voting period must be less than or equal to max voting period")]
    InvalidMinVotingPeriod {},

    #[error("{0}")]
    Tag(#[from] TagError),
}
