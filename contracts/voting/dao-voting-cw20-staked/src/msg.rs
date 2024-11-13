use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Uint128;
use cw20::Cw20Coin;
use cw20_base::msg::InstantiateMarketingInfo;
use cw_utils::Duration;

use dao_dao_macros::{active_query, cw20_token_query, voting_module_query};
use dao_voting::threshold::{ActiveThreshold, ActiveThresholdResponse};

/// Information about the staking contract to be used with this voting
/// module.
#[cw_serde]
pub enum StakingInfo {
    Existing {
        /// Address of an already instantiated staking contract.
        staking_contract_address: String,
    },
    New {
        /// Code ID for staking contract to instantiate.
        staking_code_id: u64,
        /// See corresponding field in cw20-stake's
        /// instantiation. This will be used when instantiating the
        /// new staking contract.
        unstaking_duration: Option<Duration>,
    },
}

#[cw_serde]
#[allow(clippy::large_enum_variant)]
pub enum TokenInfo {
    Existing {
        /// Address of an already instantiated cw20 token contract.
        address: String,
        /// Information about the staking contract to use.
        staking_contract: StakingInfo,
    },
    New {
        /// Code ID for cw20 token contract.
        code_id: u64,
        /// Label to use for instantiated cw20 contract.
        label: String,

        name: String,
        symbol: String,
        decimals: u8,
        initial_balances: Vec<Cw20Coin>,
        marketing: Option<InstantiateMarketingInfo>,

        staking_code_id: u64,
        unstaking_duration: Option<Duration>,
        initial_dao_balance: Option<Uint128>,
    },
}

#[cw_serde]
pub struct InstantiateMsg {
    pub token_info: TokenInfo,
    /// The number or percentage of tokens that must be staked
    /// for the DAO to be active
    pub active_threshold: Option<ActiveThreshold>,
}

#[cw_serde]
#[derive(cw_orch::ExecuteFns)]
pub enum ExecuteMsg {
    /// Sets the active threshold to a new value. Only the
    /// instantiator this contract (a DAO most likely) may call this
    /// method.
    UpdateActiveThreshold {
        new_threshold: Option<ActiveThreshold>,
    },
}

#[voting_module_query]
#[cw20_token_query]
#[active_query]
#[cw_serde]
#[derive(QueryResponses, cw_orch::QueryFns)]
pub enum QueryMsg {
    /// Gets the address of the cw20-stake contract this voting module
    /// is wrapping.
    #[returns(cosmwasm_std::Addr)]
    StakingContract {},
    #[returns(ActiveThresholdResponse)]
    ActiveThreshold {},
}

#[cw_serde]
pub struct MigrateMsg {}
