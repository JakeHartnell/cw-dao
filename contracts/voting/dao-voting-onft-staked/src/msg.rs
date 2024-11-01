use cosmwasm_schema::{cw_serde, QueryResponses};
use cw_utils::{Duration, Expiration};
use dao_dao_macros::{active_query, voting_module_query};
use dao_voting::threshold::{ActiveThreshold, ActiveThresholdResponse};

#[cw_serde]
#[allow(clippy::large_enum_variant)]
pub enum OnftCollection {
    /// Uses an existing x/onft denom/collection.
    Existing {
        /// ID of an already created x/onft denom/collection.
        id: String,
    },
}

#[cw_serde]
pub struct InstantiateMsg {
    /// ONFT collection that will be staked.
    pub onft_collection: OnftCollection,
    /// Amount of time between unstaking and tokens being available. To unstake
    /// with no delay, leave as `None`.
    pub unstaking_duration: Option<Duration>,
    /// The number or percentage of tokens that must be staked for the DAO to be
    /// active
    pub active_threshold: Option<ActiveThreshold>,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Step 1/3 of the NFT staking process. x/onft doesn't support executing a
    /// smart contract on NFT transfer like cw721s do, so the stake process is
    /// broken up:
    /// 1. The sender calls `PrepareStake` to inform this staking contract of
    ///    the NFTs that are about to be staked. This will succeed only if the
    ///    sender currently owns the NFT(s).
    /// 2. The sender then transfers the NFT(s) to the staking contract.
    /// 3. The sender calls `ConfirmStake` on this staking contract which
    ///    confirms the NFTs were transferred to it and registers the stake.
    ///
    /// PrepareStake overrides any previous PrepareStake calls, as long as the
    /// sender owns the NFT(s).
    PrepareStake { token_ids: Vec<String> },
    /// Step 3/3 of the NFT staking process. x/onft doesn't support executing a
    /// smart contract on NFT transfer like cw721s do, so the stake process is
    /// broken up:
    /// 1. The sender calls `PrepareStake` to inform this staking contract of
    ///    the NFTs that are about to be staked. This will succeed only if the
    ///    sender currently owns the NFT(s).
    /// 2. The sender then transfers the NFT(s) to the staking contract.
    /// 3. The sender calls `ConfirmStake` on this staking contract which
    ///    confirms the NFTs were transferred to it and registers the stake.
    ConfirmStake { token_ids: Vec<String> },
    /// CancelStake serves as an undo function in case an NFT or stake gets into
    /// a bad state, either because the stake process was never completed, or
    /// because someone sent an NFT to the staking contract without preparing
    /// the stake first.
    ///
    /// If called by:
    /// - the original stake preparer, the preparation will be canceled, and the
    ///   NFT(s) will be sent back if the staking contract owns them.
    /// - the current NFT(s) owner, the preparation will be canceled, if any.
    /// - the DAO, the preparation will be canceled (if any exists), and the
    ///   NFT(s) will be sent to the specified recipient (if the staking
    ///   contract owns them). if no recipient is specified but the NFT was
    ///   prepared, it will be sent back to the preparer.
    ///
    /// The recipient field only applies when the sender is the DAO. In the
    /// other cases, the NFT(s) will always be sent back to the sender. Note: if
    /// the NFTs were sent to the staking contract, but no stake was prepared,
    /// only the DAO will be able to correct this and send them somewhere.
    CancelStake {
        token_ids: Vec<String>,
        recipient: Option<String>,
    },
    /// Unstakes the specified token_ids on behalf of the sender. token_ids must
    /// have unique values and have non-zero length.
    Unstake { token_ids: Vec<String> },
    /// Claim NFTs that have been unstaked for the specified duration.
    ClaimNfts { r#type: ClaimType },
    /// Updates the contract configuration, namely unstaking duration. Only
    /// callable by the DAO that initialized this voting contract.
    UpdateConfig { duration: Option<Duration> },
    /// Adds a hook which is called on staking / unstaking events. Only callable
    /// by the DAO that initialized this voting contract.
    AddHook { addr: String },
    /// Removes a hook which is called on staking / unstaking events. Only
    /// callable by the DAO that initialized this voting contract.
    RemoveHook { addr: String },
    /// Sets the active threshold to a new value. Only callable by the DAO that
    /// initialized this voting contract.
    UpdateActiveThreshold {
        new_threshold: Option<ActiveThreshold>,
    },
}

#[cw_serde]
pub enum ClaimType {
    /// Claims all legacy claims.
    Legacy,
    /// Claims all non-legacy claims.
    All,
    /// Claims specific non-legacy NFTs.
    Specific(Vec<String>),
}

#[active_query]
#[voting_module_query]
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(crate::state::Config)]
    Config {},
    #[returns(NftClaimsResponse)]
    NftClaims {
        address: String,
        start_after: Option<String>,
        limit: Option<u32>,
    },
    #[returns(::cw_controllers::HooksResponse)]
    Hooks {},
    // List the staked NFTs for a given address.
    #[returns(Vec<String>)]
    StakedNfts {
        address: String,
        start_after: Option<String>,
        limit: Option<u32>,
    },
    #[returns(ActiveThresholdResponse)]
    ActiveThreshold {},
}

#[cw_serde]
pub struct NftClaimsResponse {
    pub nft_claims: Vec<NftClaim>,
}

#[cw_serde]
pub struct NftClaim {
    /// The token ID of the NFT being claimed.
    pub token_id: String,
    /// The expiration time of the claim.
    pub release_at: Expiration,
    /// Whether the claim is a legacy claim.
    pub legacy: bool,
}

#[cw_serde]
pub struct MigrateMsg {}
