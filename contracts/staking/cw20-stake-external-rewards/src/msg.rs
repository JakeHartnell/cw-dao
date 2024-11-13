use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Uint128;
use cw20::{Cw20ReceiveMsg, Denom};
use dao_hooks::stake::StakeChangedHookMsg;

use crate::state::{Config, RewardConfig};

pub use cw_controllers::ClaimsResponse;
// so that consumers don't need a cw_ownable dependency to consume
// this contract's queries.
pub use cw_ownable::Ownership;

use cw_ownable::cw_ownable_execute;

#[cw_serde]
pub struct InstantiateMsg {
    pub owner: Option<String>,
    pub staking_contract: String,
    pub reward_token: Denom,
    pub reward_duration: u64,
}

#[cw_ownable_execute]
#[cw_serde]
#[derive(cw_orch::ExecuteFns)]
pub enum ExecuteMsg {
    StakeChangeHook(StakeChangedHookMsg),
    Claim {},
    Receive(Cw20ReceiveMsg),
    Fund {},
    UpdateRewardDuration { new_duration: u64 },
}

#[cw_serde]
pub enum MigrateMsg {
    /// Migrates from version 0.2.6 to 2.0.0. The significant changes
    /// being the addition of a two-step ownership transfer using
    /// `cw_ownable` and the removal of the manager. Migrating will
    /// automatically remove the current manager.
    FromV1 {},
}

#[cw_serde]
pub enum ReceiveMsg {
    Fund {},
}

#[cw_serde]
#[derive(QueryResponses, cw_orch::QueryFns)]
pub enum QueryMsg {
    #[returns(InfoResponse)]
    Info {},
    #[returns(PendingRewardsResponse)]
    GetPendingRewards { address: String },
    #[returns(::cw_ownable::Ownership<::cosmwasm_std::Addr>)]
    Ownership {},
}

#[cw_serde]
pub struct InfoResponse {
    pub config: Config,
    pub reward: RewardConfig,
}

#[cw_serde]
pub struct PendingRewardsResponse {
    pub address: String,
    pub pending_rewards: Uint128,
    pub denom: Denom,
    pub last_update_block: u64,
}
