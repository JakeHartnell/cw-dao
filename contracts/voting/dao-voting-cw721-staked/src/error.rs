use cosmwasm_std::{Addr, StdError};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error(transparent)]
    Std(#[from] StdError),

    #[error("Can not stake that which has already been staked")]
    AlreadyStaked {},

    #[error(transparent)]
    HookError(#[from] cw_controllers::HookError),

    #[error("Invalid token. Got ({received}), expected ({expected})")]
    InvalidToken { received: Addr, expected: Addr },

    #[error("Error instantiating cw721-roles contract")]
    NftInstantiateError {},

    #[error("New cw721-roles contract must be instantiated with at least one NFT")]
    NoInitialNfts {},

    #[error("Nothing to claim")]
    NothingToClaim {},

    #[error("Only the owner of this contract my execute this message")]
    NotOwner {},

    #[error("Can not unstake that which you have not staked (unstaking {token_id})")]
    NotStaked { token_id: String },

    #[error("Too many outstanding claims. Claim some tokens before unstaking more.")]
    TooManyClaims {},

    #[error("Got a submessage reply with unknown id: {id}")]
    UnknownReplyId { id: u64 },

    #[error("Can't unstake zero NFTs.")]
    ZeroUnstake {},
}
