use cosmwasm_std::{OverflowError, StdError};
use cw_hooks::HookError;
use cw_utils::ParseReplyError;
use dao_voting::{reply::error::TagError, veto::VetoError};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error(transparent)]
    Std(#[from] StdError),

    #[error(transparent)]
    ParseReplyError(#[from] ParseReplyError),

    #[error(transparent)]
    HookError(#[from] HookError),

    #[error(transparent)]
    VetoError(#[from] VetoError),

    #[error(transparent)]
    OverflowError(#[from] OverflowError),

    #[error("unauthorized")]
    Unauthorized {},

    #[error(transparent)]
    ThresholdError(#[from] dao_voting::threshold::ThresholdError),

    #[error(transparent)]
    VotingError(#[from] dao_voting::error::VotingError),

    #[error("no such proposal ({id})")]
    NoSuchProposal { id: u64 },

    #[error("no vote exists for proposal ({id}) and voter ({voter})")]
    NoSuchVote { id: u64, voter: String },

    #[error("proposal is ({size}) bytes, must be <= ({max}) bytes")]
    ProposalTooLarge { size: u64, max: u64 },

    #[error("Proposal ({id}) is expired")]
    Expired { id: u64 },

    #[error("not registered to vote (no voting power) at time of proposal creation")]
    NotRegistered {},

    #[error("already voted. this proposal does not support revoting")]
    AlreadyVoted {},

    #[error("already cast a vote with that option. change your vote to revote")]
    AlreadyCast {},

    #[error("proposal is not in 'passed' state")]
    NotPassed {},

    #[error("proposal has already been executed")]
    AlreadyExecuted {},

    #[error("proposal is closed")]
    Closed {},

    #[error("only rejected proposals may be closed")]
    WrongCloseStatus {},

    #[error("the DAO is currently inactive, you cannot create proposals")]
    InactiveDao {},

    #[error("min_voting_period and max_voting_period must have the same units (height or time)")]
    DurationUnitsConflict {},

    #[error("min voting period must be less than or equal to max voting period")]
    InvalidMinVotingPeriod {},

    #[error(
        "pre-propose modules must specify a proposer. lacking one, no proposer should be specified"
    )]
    InvalidProposer {},

    #[error(transparent)]
    Tag(#[from] TagError),

    #[error(
        "all proposals with deposits must be completed out (closed or executed) before migration"
    )]
    PendingProposals {},

    #[error("received a failed proposal hook reply with an invalid hook index: ({idx})")]
    InvalidHookIndex { idx: u64 },

    #[error("received a reply failure with an invalid ID: ({id})")]
    InvalidReplyID { id: u64 },

    #[error("can not migrate. current version is up to date")]
    AlreadyMigrated {},

    #[error("incompatible migration version")]
    MigrationVersionError {},
}
