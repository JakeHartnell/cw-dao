use cosmwasm_std::{Addr, StdError};
use cw_utils::ParseReplyError;
use dao_voting::threshold::ActiveThresholdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error(transparent)]
    Std(#[from] StdError),

    #[error(transparent)]
    ActiveThresholdError(#[from] ActiveThresholdError),

    #[error(transparent)]
    HookError(#[from] cw_hooks::HookError),

    #[error(transparent)]
    ParseReplyError(#[from] ParseReplyError),

    #[error(transparent)]
    UnstakingDurationError(#[from] dao_voting::duration::UnstakingDurationError),

    #[error(transparent)]
    NftClaimError(#[from] cw721_controllers::NftClaimError),

    #[error("Can not stake that which has already been staked")]
    AlreadyStaked {},

    #[error("Invalid token. Got ({received}), expected ({expected})")]
    InvalidToken { received: Addr, expected: Addr },

    #[error("Error instantiating NFT contract")]
    NftInstantiateError {},

    #[error("New NFT contract must be instantiated with at least one NFT")]
    NoInitialNfts {},

    #[error("Factory contract did not implment the required NftFactoryCallback interface")]
    NoFactoryCallback {},

    #[error("Nothing to claim")]
    NothingToClaim {},

    #[error("Only the owner of this contract may execute this message")]
    NotOwner {},

    #[error("Can not unstake that which you have not staked (unstaking {token_id})")]
    NotStaked { token_id: String },

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Got a submessage reply with unknown id: {id}")]
    UnknownReplyId { id: u64 },

    #[error("Factory message must serialize to WasmMsg::Execute")]
    UnsupportedFactoryMsg {},

    #[error("Can't unstake zero NFTs.")]
    ZeroUnstake {},
}
