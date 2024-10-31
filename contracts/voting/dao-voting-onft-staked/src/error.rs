use cosmwasm_std::StdError;
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
    UnstakingDurationError(#[from] dao_voting::duration::UnstakingDurationError),

    #[error(transparent)]
    NftClaimError(#[from] cw721_controllers::NftClaimError),

    #[error("Nothing to claim")]
    NothingToClaim {},

    #[error("Only an NFT's owner can prepare it to be staked")]
    OnlyOwnerCanPrepareStake {},

    #[error("NFTs must be prepared and transferred before they can be staked")]
    StakeMustBePrepared {},

    #[error("Recipient must be set when the DAO is cancelling a stake that was not prepared")]
    NoRecipient {},

    #[error("Only the owner or preparer can cancel a prepared stake")]
    NotPreparerNorOwner {},

    #[error("Can not unstake that which you have not staked (unstaking {token_id})")]
    NotStaked { token_id: String },

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Got a submessage reply with unknown id: {id}")]
    UnknownReplyId { id: u64 },

    #[error("Can't unstake zero NFTs.")]
    ZeroUnstake {},
}
