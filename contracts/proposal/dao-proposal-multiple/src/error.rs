use cosmwasm_std::{OverflowError, StdError};
use cw_hooks::HookError;
use cw_utils::ParseReplyError;
use dao_voting::{reply::error::TagError, threshold::ThresholdError, veto::VetoError};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error(transparent)]
    ParseReplyError(#[from] ParseReplyError),

    #[error("{0}")]
    HookError(#[from] HookError),

    #[error(transparent)]
    VetoError(#[from] VetoError),

    #[error(transparent)]
    OverflowError(#[from] OverflowError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("{0}")]
    ThresholdError(#[from] ThresholdError),

    #[error("{0}")]
    VotingError(#[from] dao_voting::error::VotingError),

    #[error("Suggested proposal expiration is larger than the maximum proposal duration")]
    InvalidExpiration {},

    #[error("No such proposal ({id})")]
    NoSuchProposal { id: u64 },

    #[error("Proposal is ({size}) bytes, must be <= ({max}) bytes")]
    ProposalTooLarge { size: u64, max: u64 },

    #[error("Proposal ({id}) is expired")]
    Expired { id: u64 },

    #[error("Not registered to vote (no voting power) at time of proposal creation.")]
    NotRegistered {},

    #[error("No vote exists for proposal ({id}) and voter ({voter})")]
    NoSuchVote { id: u64, voter: String },

    #[error("Already voted. This proposal does not support revoting.")]
    AlreadyVoted {},

    #[error("Already cast a vote with that option. Change your vote to revote.")]
    AlreadyCast {},

    #[error("Proposal must be in 'passed' state to be executed.")]
    NotPassed {},

    #[error("Proposal is in a tie: two or more options have the same number of votes.")]
    Tie {},

    #[error("Proposal is not expired.")]
    NotExpired {},

    #[error("Only rejected proposals may be closed.")]
    WrongCloseStatus {},

    #[error("The DAO is currently inactive, you cannot create proposals.")]
    InactiveDao {},

    #[error("Proposal must have at least two choices.")]
    WrongNumberOfChoices {},

    #[error("Must have exactly one 'none of the above' option.")]
    NoneOption {},

    #[error("No vote weights found.")]
    NoVoteWeights {},

    #[error("Invalid vote selected.")]
    InvalidVote {},

    #[error("Must have voting power to propose.")]
    MustHaveVotingPower {},

    #[error(
        "pre-propose modules must specify a proposer. lacking one, no proposer should be specified"
    )]
    InvalidProposer {},

    #[error("{0}")]
    Tag(#[from] TagError),

    #[error(
        "all proposals with deposits must be completed out (closed or executed) before migration"
    )]
    PendingProposals {},

    #[error("received a failed proposal hook reply with an invalid hook index: ({idx})")]
    InvalidHookIndex { idx: u64 },

    #[error("received a reply failure with an invalid ID: ({id})")]
    InvalidReplyID { id: u64 },
}
