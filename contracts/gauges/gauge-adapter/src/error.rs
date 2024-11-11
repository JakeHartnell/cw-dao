use cosmwasm_std::{StdError, Uint128};
use cw_denom::DenomError;
use cw_utils::PaymentError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    PaymentError(#[from] PaymentError),

    #[error("{0}")]
    DenomError(#[from] DenomError),

    #[error("{0}")]
    Ownership(#[from] cw_ownable::OwnershipError),

    #[error("Operation unauthorized - only admin can release deposits")]
    Unauthorized {},

    #[error("Operation unauthorized - there's already existing submission for that destination address; only previous sender can overwrite it")]
    UnauthorizedSubmission {},

    #[error("Invalid submission - required deposit set in incorrect denom")]
    InvalidDepositType {},

    #[error("Invalid submission - invalid amount for required deposit. Either multiple denoms were sent or amount does not match {correct_amount}")]
    InvalidDepositAmount { correct_amount: Uint128 },

    #[error("No deposit was required, therefore no deposit can be returned")]
    NoDepositToRefund {},

    #[error("Deposit required, cannot create submission.")]
    DepositRequired {},
}
