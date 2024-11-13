use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Binary;
use cw721::{Cw721ReceiveMsg, Expiration};
use cw_utils::Duration;
use dao_dao_macros::{active_query, voting_module_query};
use dao_voting::threshold::{ActiveThreshold, ActiveThresholdResponse};

#[cw_serde]
#[allow(clippy::large_enum_variant)]
pub enum NftContract {
    /// Uses an existing cw721 or sg721 token contract.
    Existing {
        /// Address of an already instantiated cw721 or sg721 token contract.
        address: String,
    },
    /// Creates a new NFT collection used for staking and governance.
    New {
        /// Code ID for cw721 token contract.
        code_id: u64,
        /// Label to use for instantiated cw721 contract.
        label: String,
        msg: Binary,
        /// Initial NFTs to mint when creating the NFT contract.
        /// If empty, an error is thrown. The binary should be a
        /// valid mint message for the corresponding cw721 contract.
        initial_nfts: Vec<Binary>,
    },
    /// Uses a factory contract that must return the address of the NFT contract.
    /// The binary must serialize to a `WasmMsg::Execute` message.
    /// Validation happens in the factory contract itself, so be sure to use a
    /// trusted factory contract.
    Factory(Binary),
}

#[cw_serde]
pub struct InstantiateMsg {
    /// Address of the cw721 NFT contract that may be staked.
    pub nft_contract: NftContract,
    /// Amount of time between unstaking and tokens being
    /// avaliable. To unstake with no delay, leave as `None`.
    pub unstaking_duration: Option<Duration>,
    /// The number or percentage of tokens that must be staked
    /// for the DAO to be active
    pub active_threshold: Option<ActiveThreshold>,
}

#[cw_serde]
#[derive(cw_orch::ExecuteFns)]
pub enum ExecuteMsg {
    /// Used to stake NFTs. To stake a NFT send a cw721 send message
    /// to this contract with the NFT you would like to stake. The
    /// `msg` field is ignored.
    ReceiveNft(Cw721ReceiveMsg),
    /// Unstakes the specified token_ids on behalf of the
    /// sender. token_ids must have unique values and have non-zero
    /// length.
    Unstake { token_ids: Vec<String> },
    /// Claim NFTs that have been unstaked for the specified duration.
    ClaimNfts { r#type: ClaimType },
    /// Updates the contract configuration, namely unstaking duration.
    /// Only callable by the DAO that initialized this voting contract.
    UpdateConfig { duration: Option<Duration> },
    /// Adds a hook which is called on staking / unstaking events.
    /// Only callable by the DAO that initialized this voting contract.
    AddHook { addr: String },
    /// Removes a hook which is called on staking / unstaking events.
    /// Only callable by the DAO that initialized this voting contract.
    RemoveHook { addr: String },
    /// Sets the active threshold to a new value.
    /// Only callable by the DAO that initialized this voting contract.
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
#[derive(QueryResponses, cw_orch::QueryFns)]
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
