use crate::state::BeforeSendHookInfo;
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Coin, Uint128};

#[cfg(any(feature = "osmosis_tokenfactory", feature = "cosmwasm_tokenfactory"))]
pub use dao_interface::token::{DenomUnit, Metadata};

/// The message used to create a new instance of this smart contract.
#[cw_serde]
pub enum InstantiateMsg {
    /// `NewToken` will create a new token when instantiate the contract.
    /// Newly created token will have full denom as `factory/<contract_address>/<subdenom>`.
    /// It will be attached to the contract setup the beforesend listener automatically.
    NewToken {
        /// component of fulldenom (`factory/<contract_address>/<subdenom>`).
        subdenom: String,
    },
    /// `ExistingToken` will use already created token. So to set this up,
    /// Token Factory admin for the existing token needs trasfer admin over
    /// to this contract, and optionally set the `BeforeSendHook` manually.
    ExistingToken { denom: String },
}

/// State changing methods available to this smart contract.
#[cw_serde]
#[derive(cw_orch::ExecuteFns)]
pub enum ExecuteMsg {
    /// Allow adds the target address to the allowlist to be able to send or recieve tokens even if the token
    /// is frozen. Token Factory's BeforeSendHook listener must be set to this contract in order for this feature
    /// to work.
    ///
    /// This functionality is intedended for DAOs who do not wish to have a their tokens liquid while bootstrapping
    /// their DAO. For example, a DAO may wish to white list a Token Staking contract (to allow users to stake their
    /// tokens in the DAO) or a Merkle Drop contract (to allow users to claim their tokens).
    Allow { address: String, status: bool },

    /// Burn token to address. Burn allowance is required and wiil be deducted after successful burn.
    Burn {
        from_address: String,
        amount: Uint128,
    },

    /// Mint token to address. Mint allowance is required and wiil be deducted after successful mint.
    Mint { to_address: String, amount: Uint128 },

    /// Deny adds the target address to the denylist, whis prevents them from sending/receiving the token attached
    /// to this contract tokenfactory's BeforeSendHook listener must be set to this contract in order for this
    /// feature to work as intended.
    Deny { address: String, status: bool },

    /// Block every token transfers of the token attached to this contract.
    /// Token Factory's BeforeSendHook listener must be set to this contract in order for this
    /// feature to work as intended.
    Freeze { status: bool },

    /// Force transfer token from one address to another.
    #[cfg(feature = "osmosis_tokenfactory")]
    ForceTransfer {
        amount: Uint128,
        from_address: String,
        to_address: String,
    },

    /// Attempt to SetBeforeSendHook on the token attached to this contract.
    /// This will fail if the chain does not support bank module hooks (many Token
    /// Factory implementations do not yet support).
    ///
    /// This takes a cosmwasm_address as an argument, which is the address of the
    /// contract that will be called before every token transfer. Normally, this
    /// will be the issuer contract itself, though it can be a custom contract for
    /// greater flexibility.
    ///
    /// Setting the address to an empty string will remove the SetBeforeSendHook.
    ///
    /// This method can only be called by the contract owner.
    #[cfg(feature = "osmosis_tokenfactory")]
    SetBeforeSendHook { cosmwasm_address: String },

    /// Grant/revoke burn allowance.
    SetBurnerAllowance { address: String, allowance: Uint128 },

    /// Set denom metadata. see: https://docs.cosmos.network/main/modules/bank#denom-metadata.
    #[cfg(any(feature = "osmosis_tokenfactory", feature = "cosmwasm_tokenfactory"))]
    SetDenomMetadata { metadata: Metadata },

    /// Grant/revoke mint allowance.
    SetMinterAllowance { address: String, allowance: Uint128 },

    /// Updates the admin of the Token Factory token.
    /// Normally this is the cw-tokenfactory-issuer contract itself.
    /// This is intended to be used only if you seek to transfer ownership
    /// of the Token somewhere else (i.e. to another management contract).
    UpdateTokenFactoryAdmin { new_admin: String },

    /// Updates the owner of this contract who is allowed to call privileged methods.
    /// NOTE: this is separate from the Token Factory token admin, for this contract to work
    /// at all, it needs to the be the Token Factory token admin.
    ///
    /// Normally, the contract owner will be a DAO.
    ///
    /// The `action` to be provided can be either to propose transferring ownership to an
    /// account, accept a pending ownership transfer, or renounce the ownership permanently.
    UpdateOwnership(cw_ownable::Action),
}

/// Used for smart contract migration.
#[cw_serde]
pub struct MigrateMsg {}

/// Queries supported by this smart contract.
#[cw_serde]
#[derive(QueryResponses, cw_orch::QueryFns)]
pub enum QueryMsg {
    /// Returns if token transfer is disabled. Response: IsFrozenResponse
    #[returns(IsFrozenResponse)]
    IsFrozen {},

    /// Returns the token denom that this contract is the admin for. Response: DenomResponse
    #[returns(DenomResponse)]
    Denom {},

    #[returns(::cw_ownable::Ownership<::cosmwasm_std::Addr>)]
    Ownership {},

    /// Returns the burn allowance of the specified address. Response: AllowanceResponse
    #[returns(AllowanceResponse)]
    BurnAllowance { address: String },

    /// Enumerates over all burn allownances. Response: AllowancesResponse
    #[returns(AllowancesResponse)]
    BurnAllowances {
        start_after: Option<String>,
        limit: Option<u32>,
    },

    /// Returns the mint allowance of the specified user. Response: AllowanceResponse
    #[returns(AllowanceResponse)]
    MintAllowance { address: String },

    /// Enumerates over all mint allownances. Response: AllowancesResponse
    #[returns(AllowancesResponse)]
    MintAllowances {
        start_after: Option<String>,
        limit: Option<u32>,
    },

    /// Returns wether the user is on denylist or not. Response: StatusResponse
    #[returns(StatusResponse)]
    IsDenied { address: String },

    /// Enumerates over all addresses on the denylist. Response: DenylistResponse
    #[returns(DenylistResponse)]
    Denylist {
        start_after: Option<String>,
        limit: Option<u32>,
    },

    /// Returns wether the user is on the allowlist or not. Response: StatusResponse
    #[returns(StatusResponse)]
    IsAllowed { address: String },

    /// Enumerates over all addresses on the allowlist. Response: AllowlistResponse
    #[returns(AllowlistResponse)]
    Allowlist {
        start_after: Option<String>,
        limit: Option<u32>,
    },

    /// Returns information about the BeforeSendHook for the token. Note: many Token
    /// Factory chains do not yet support this feature.
    ///
    /// The information returned is:
    /// - Whether features in this contract that require MsgBeforeSendHook are enabled.
    /// - The address of the BeforeSendHook contract if configured.
    ///
    /// Response: BeforeSendHookInfo
    #[returns(BeforeSendHookInfo)]
    BeforeSendHookInfo {},
}

/// SudoMsg is only exposed for internal Cosmos SDK modules to call.
/// This is showing how we can expose "admin" functionality than can not be called by
/// external users or contracts, but only trusted (native/Go) code in the blockchain
#[cw_serde]
pub enum SudoMsg {
    BlockBeforeSend {
        from: String,
        to: String,
        amount: Coin,
    },
}

/// Returns whether or not the Token Factory token is frozen and transfers
/// are disabled.
#[cw_serde]
pub struct IsFrozenResponse {
    pub is_frozen: bool,
}

/// Returns the full denomination for the Token Factory token. For example:
/// `factory/{contract address}/{subdenom}`
#[cw_serde]
pub struct DenomResponse {
    pub denom: String,
}

/// Returns the current owner of this issuer contract who is allowed to
/// call priviledged methods.
#[cw_serde]
pub struct OwnerResponse {
    pub address: String,
}

/// Returns a mint or burn allowance for a particular address, representing
/// the amount of tokens the account is allowed to mint or burn
#[cw_serde]
pub struct AllowanceResponse {
    pub allowance: Uint128,
}

/// Information about a particular account and its mint / burn allowances.
/// Used in list queries.
#[cw_serde]
pub struct AllowanceInfo {
    pub address: String,
    pub allowance: Uint128,
}

/// Returns a list of all mint or burn allowances
#[cw_serde]
pub struct AllowancesResponse {
    pub allowances: Vec<AllowanceInfo>,
}

/// Whether a particular account is allowed or denied
#[cw_serde]
pub struct StatusResponse {
    pub status: bool,
}

/// Account info for list queries related to allowlist and denylist
#[cw_serde]
pub struct StatusInfo {
    pub address: String,
    pub status: bool,
}

/// Returns a list of addresses currently on the denylist.
#[cw_serde]
pub struct DenylistResponse {
    pub denylist: Vec<StatusInfo>,
}

/// Returns a list of addresses currently on the allowlist
#[cw_serde]
pub struct AllowlistResponse {
    pub allowlist: Vec<StatusInfo>,
}
