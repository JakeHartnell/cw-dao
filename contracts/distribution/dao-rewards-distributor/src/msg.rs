use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Uint128;
use cw20::{Cw20ReceiveMsg, Denom, UncheckedDenom};
use cw4::MemberChangedHookMsg;
use cw_ownable::cw_ownable_execute;
use dao_hooks::{nft_stake::NftStakeChangedHookMsg, stake::StakeChangedHookMsg};
use dao_interface::voting::InfoResponse;

// so that consumers don't need a cw_ownable or cw_controllers dependency
// to consume this contract's queries.
pub use cw_controllers::ClaimsResponse;
pub use cw_ownable::Ownership;

use crate::state::{DistributionState, EmissionRate};

#[cw_serde]
pub struct InstantiateMsg {
    /// The owner of the contract. Is able to fund the contract and update the
    /// reward duration. If not provided, the instantiator is used.
    pub owner: Option<String>,
}

#[cw_ownable_execute]
#[cw_serde]
pub enum ExecuteMsg {
    /// Called when a member is added or removed
    /// to a cw4-groups or cw721-roles contract.
    MemberChangedHook(MemberChangedHookMsg),
    /// Called when NFTs are staked or unstaked.
    NftStakeChangeHook(NftStakeChangedHookMsg),
    /// Called when tokens are staked or unstaked.
    StakeChangeHook(StakeChangedHookMsg),
    /// registers a new distribution
    Create(CreateMsg),
    /// updates the config for a distribution
    Update {
        /// distribution ID to update
        id: u64,
        /// reward emission rate
        emission_rate: Option<EmissionRate>,
        /// address to query the voting power
        vp_contract: Option<String>,
        /// address that will update the reward split when the voting power
        /// distribution changes
        hook_caller: Option<String>,
        /// whether or not non-owners can fund the distribution
        open_funding: Option<bool>,
        /// destination address for reward clawbacks. defaults to owner
        withdraw_destination: Option<String>,
    },
    /// Used to fund this contract with cw20 tokens.
    Receive(Cw20ReceiveMsg),
    /// Used to fund this contract with native tokens.
    Fund(FundMsg),
    /// Used to fund the latest distribution with native tokens.
    FundLatest {},
    /// Claims rewards for the sender.
    Claim { id: u64 },
    /// withdraws the undistributed rewards for a distribution. members can
    /// claim whatever they earned until this point. this is effectively an
    /// inverse to fund and does not affect any already-distributed rewards.
    Withdraw { id: u64 },
    /// forcibly withdraw funds from the contract. this is unsafe and should
    /// only be used to recover funds that are stuck in the contract.
    UnsafeForceWithdraw {
        /// amount to withdraw
        amount: Uint128,
        /// denom to withdraw
        denom: UncheckedDenom,
    },
}

#[cw_serde]
pub struct CreateMsg {
    /// denom to distribute
    pub denom: UncheckedDenom,
    /// reward emission rate
    pub emission_rate: EmissionRate,
    /// address to query the voting power
    pub vp_contract: String,
    /// address that will update the reward split when the voting power
    /// distribution changes
    pub hook_caller: String,
    /// whether or not non-owners can fund the distribution. defaults to true.
    pub open_funding: Option<bool>,
    /// destination address for reward clawbacks. defaults to owner.
    pub withdraw_destination: Option<String>,
}

#[cw_serde]
pub struct FundMsg {
    /// distribution ID to fund
    pub id: u64,
}

#[cw_serde]
pub enum ReceiveCw20Msg {
    /// Used to fund this contract with cw20 tokens.
    Fund(FundMsg),
    /// Used to fund the latest distribution with cw20 tokens. We can't verify
    /// the sender of CW20 token send contract executions; since the create
    /// function is restricted to the contract owner, we cannot allow creating
    /// new distributions and funding with CW20 tokens in one message (like we
    /// can with native tokens via the funds field). To prevent DAOs from having
    /// to submit two proposals to create+fund a CW20 distribution, we allow
    /// creating and funding a distribution in one transaction via this message
    /// that funds the latest distribution without knowing the ID ahead of time.
    FundLatest {},
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Returns contract version info
    #[returns(InfoResponse)]
    Info {},
    /// Returns information about the ownership of this contract.
    #[returns(::cw_ownable::Ownership<::cosmwasm_std::Addr>)]
    Ownership {},
    /// Returns the pending rewards for the given address.
    #[returns(PendingRewardsResponse)]
    PendingRewards {
        address: String,
        start_after: Option<u64>,
        limit: Option<u32>,
    },
    /// Returns the undistributed rewards for a distribution.
    #[returns(Uint128)]
    UndistributedRewards { id: u64 },
    /// Returns the state of the given distribution.
    #[returns(DistributionState)]
    Distribution { id: u64 },
    /// Returns the state of all the distributions.
    #[returns(DistributionsResponse)]
    Distributions {
        start_after: Option<u64>,
        limit: Option<u32>,
    },
}

#[cw_serde]
pub struct DistributionsResponse {
    pub distributions: Vec<DistributionState>,
}

#[cw_serde]
pub struct PendingRewardsResponse {
    pub pending_rewards: Vec<DistributionPendingRewards>,
}

#[cw_serde]
pub struct DistributionPendingRewards {
    /// distribution ID
    pub id: u64,
    /// denomination of the pending rewards
    pub denom: Denom,
    /// amount of pending rewards in the denom being distributed
    pub pending_rewards: Uint128,
}

#[cw_serde]
pub struct MigrateMsg {}
