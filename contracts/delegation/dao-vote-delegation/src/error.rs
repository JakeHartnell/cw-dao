use cosmwasm_std::{DivideByZeroError, OverflowError, StdError};
use cw_utils::PaymentError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error(transparent)]
    Std(#[from] StdError),

    #[error(transparent)]
    Overflow(#[from] OverflowError),

    #[error(transparent)]
    DivideByZero(#[from] DivideByZeroError),

    #[error(transparent)]
    Payment(#[from] PaymentError),

    #[error("semver parsing error: {0}")]
    SemVer(String),

    #[error("delegation module not setup. ensure voting power hook callers are registered and proposal modules are synced.")]
    DelegationModuleNotSetup {},

    #[error("unauthorized")]
    Unauthorized {},

    #[error("unauthorized hook caller")]
    UnauthorizedHookCaller {},

    #[error("invalid delegation validity blocks: provided {provided}, minimum {min}")]
    InvalidDelegationValidityBlocks { provided: u64, min: u64 },

    #[error("delegate already registered")]
    DelegateAlreadyRegistered {},

    #[error("delegate not registered")]
    DelegateNotRegistered {},

    #[error("delegates cannot delegate to others")]
    DelegatesCannotDelegate {},

    #[error("cannot register as a delegate with existing delegations")]
    CannotRegisterWithDelegations {},

    #[error("no voting power")]
    NoVotingPower {},

    #[error("delegation does not exist")]
    DelegationDoesNotExist {},

    #[error("cannot delegate more than 100% (current: {current}%, attempt: {attempt}%)")]
    CannotDelegateMoreThan100Percent { current: String, attempt: String },

    #[error("invalid voting power percent")]
    InvalidVotingPowerPercent {},

    #[error("cannot delegate more than {max} delegations (current: {current})")]
    MaxDelegationsReached { max: u64, current: usize },

    #[error("migration error: incorrect contract: expected \"{expected}\", actual \"{actual}\"")]
    MigrationErrorIncorrectContract { expected: String, actual: String },

    #[error("migration error: invalid version: new {new}, current {current}")]
    MigrationErrorInvalidVersion { new: String, current: String },
}

impl From<semver::Error> for ContractError {
    fn from(err: semver::Error) -> Self {
        Self::SemVer(err.to_string())
    }
}
