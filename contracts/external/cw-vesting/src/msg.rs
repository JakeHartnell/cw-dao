use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Timestamp, Uint128};
use cw20::Cw20ReceiveMsg;
use cw_denom::UncheckedDenom;
use cw_ownable::cw_ownable_execute;
use cw_stake_tracker::StakeTrackerQuery;

use crate::vesting::Schedule;

#[cw_serde]
pub struct InstantiateMsg {
    /// The optional owner address of the contract. If an owner is
    /// specified, the owner may cancel the vesting contract at any
    /// time and withdraw unvested funds.
    pub owner: Option<String>,
    /// The receiver address of the vesting tokens.
    pub recipient: String,

    /// The a name or title for this payment.
    pub title: String,
    /// A description for the payment to provide more context.
    pub description: Option<String>,

    /// The total amount of tokens to be vested.
    pub total: Uint128,
    /// The type and denom of token being vested.
    pub denom: UncheckedDenom,

    /// The vesting schedule, can be either `SaturatingLinear` vesting
    /// (which vests evenly over time), or `PiecewiseLinear` which can
    /// represent a more complicated vesting schedule.
    pub schedule: Schedule,
    /// The time to start vesting, or None to start vesting when the
    /// contract is instantiated. `start_time` may be in the past,
    /// though the contract checks that `start_time +
    /// vesting_duration_seconds > now`. Otherwise, this would amount
    /// to a regular fund transfer.
    pub start_time: Option<Timestamp>,
    /// The length of the vesting schedule in seconds. Must be
    /// non-zero, though one second vesting durations are
    /// allowed. This may be combined with a `start_time` in the
    /// future to create an agreement that instantly vests at a time
    /// in the future, and allows the receiver to stake vesting tokens
    /// before the agreement completes.
    ///
    /// See `suite_tests/tests.rs`
    /// `test_almost_instavest_in_the_future` for an example of this.
    pub vesting_duration_seconds: u64,

    /// The unbonding duration for the chain this contract is deployed
    /// on. Smart contracts do not have access to this data as
    /// stargate queries are disabled on most chains, and cosmwasm-std
    /// provides no way to query it.
    ///
    /// This value being too high will cause this contract to hold
    /// funds for longer than needed, this value being too low will
    /// reduce the quality of error messages and require additional
    /// external calculations with correct values to withdraw
    /// avaliable funds from the contract.
    pub unbonding_duration_seconds: u64,
}

#[cw_ownable_execute]
#[cw_serde]
#[derive(cw_orch::ExecuteFns)]
pub enum ExecuteMsg {
    /// Fund the contract with a cw20 token. The `msg` field must have
    /// the shape `{"fund":{}}`, and the amount sent must be the same
    /// as the amount to be vested (as set during instantiation).
    /// Anyone may call this method so long as the contract has not
    /// yet been funded.
    Receive(Cw20ReceiveMsg),
    /// Distribute vested tokens to the vest receiver. Anyone may call
    /// this method.
    Distribute {
        /// The amount of tokens to distribute. If none are specified
        /// all claimable tokens will be distributed.
        amount: Option<Uint128>,
    },
    /// Cancels the vesting payment. The current amount vested becomes
    /// the total amount that will ever vest, and all pending and
    /// future staking rewards from tokens staked by this contract
    /// will be sent to the owner. Tote that canceling does not impact
    /// already vested tokens.
    ///
    /// Upon canceling, the contract will use any liquid tokens in the
    /// contract to settle pending payments to the vestee, and then
    /// returns the rest to the owner. Staked tokens are then split
    /// between the owner and the vestee according to the number of
    /// tokens that the vestee is entitled to.
    ///
    /// The vestee will no longer receive staking rewards after
    /// cancelation, and may unbond and distribute (vested - claimed)
    /// tokens at their leisure. the owner will receive staking
    /// rewards and may unbond and withdraw (staked - (vested -
    /// claimed)) tokens at their leisure.
    Cancel {},
    /// This is translated to a
    /// [MsgDelegate](https://github.com/cosmos/cosmos-sdk/blob/v0.40.0/proto/cosmos/staking/v1beta1/tx.proto#L81-L90).
    /// `delegator_address` is automatically filled with the current
    /// contract's address.  Note: this only works with the native
    /// staking denom of a Cosmos chain.  Only callable by Vesting
    /// Payment Recipient.
    #[cfg(feature = "staking")]
    Delegate {
        /// The validator to delegate to.
        validator: String,
        /// The amount to delegate.
        amount: Uint128,
    },
    /// This is translated to a
    /// [MsgBeginRedelegate](https://github.com/cosmos/cosmos-sdk/blob/v0.40.0/proto/cosmos/staking/v1beta1/tx.proto#L96).
    /// `delegator_address` is automatically filled with the current
    /// contract's address.  Only callable by Vesting Payment
    /// Recipient.
    #[cfg(feature = "staking")]
    Redelegate {
        src_validator: String,
        dst_validator: String,
        amount: Uint128,
    },
    /// This is translated to a
    /// [MsgUndelegate](https://github.com/cosmos/cosmos-sdk/blob/v0.40.0/proto/cosmos/staking/v1beta1/tx.proto#L112-L121).
    /// `delegator_address` is automatically filled with the current
    /// contract's address.  Only callable by Vesting Payment
    /// Recipient.
    #[cfg(feature = "staking")]
    Undelegate {
        /// The validator to undelegate from
        validator: String,
        /// The amount to delegate
        amount: Uint128,
    },
    /// This is translated to a
    /// [MsgSetWithdrawAddress](https://github.com/cosmos/cosmos-sdk/blob/v0.42.4/proto/cosmos/distribution/v1beta1/tx.proto#L31-L37).
    /// `delegator_address` is automatically filled with the current
    /// contract's address.  Only callable by Vesting Payment
    /// Recipient.
    #[cfg(feature = "staking")]
    SetWithdrawAddress { address: String },
    /// This is translated to a
    /// [MsgWithdrawDelegatorReward](https://github.com/cosmos/cosmos-sdk/blob/v0.42.4/proto/cosmos/distribution/v1beta1/tx.proto#L42-L50).
    /// `delegator_address` is automatically filled with the current
    /// contract's address.
    #[cfg(feature = "staking")]
    WithdrawDelegatorReward {
        /// The validator to claim rewards for.
        validator: String,
    },
    /// If the owner cancels a payment and there are not enough liquid
    /// tokens to settle the owner may become entitled to some number
    /// of staked tokens. They may then unbond those tokens and then
    /// call this method to return them.
    WithdrawCanceledPayment {
        /// The amount to withdraw.
        amount: Option<Uint128>,
    },
    /// Registers a slash event bonded or unbonding tokens with the
    /// contract. Only callable by the owner as the contract is unable
    /// to verify that the slash actually occured. The owner is
    /// assumed to be honest.
    ///
    /// A future version of this contract may be able to
    /// permissionlessly take slashing evidence:
    /// <https://github.com/CosmWasm/mesh-security/issues/35>
    #[cfg(feature = "staking")]
    RegisterSlash {
        /// The validator the slash occured for.
        validator: String,
        /// The time the slash event occured. Note that this is not
        /// validated beyond validating that it is < now. This means
        /// that if two slash events occur for a single validator, and
        /// then this method is called, a dishonest sender could
        /// register those two slashes as a single larger one at the
        /// time of the first slash.
        ///
        /// The result of this is that the staked balances tracked in
        /// this contract can not be relied on for accurate values in
        /// the past. Staked balances will be correct at time=now.
        time: Timestamp,
        /// The number of tokens that THIS CONTRACT lost as a result
        /// of the slash. Note that this differs from the total amount
        /// slashed from the validator.
        amount: Uint128,
        /// If the slash happened during unbonding. Set to false in
        /// the common case where the slash impacted bonding tokens.
        during_unbonding: bool,
    },
}

#[cw_serde]
pub enum ReceiveMsg {
    /// Funds a vesting contract with a cw20 token
    Fund {},
}

#[cw_serde]
#[derive(QueryResponses, cw_orch::QueryFns)]
pub enum QueryMsg {
    /// Get the current ownership.
    #[returns(::cw_ownable::Ownership<::cosmwasm_std::Addr>)]
    Ownership {},
    /// Returns information about the vesting contract and the
    /// status of the payment.
    #[returns(crate::vesting::Vest)]
    Info {},
    /// Returns the number of tokens currently claimable by the
    /// vestee. This is the minimum of the number of unstaked tokens
    /// in the contract, and the number of tokens that have been
    /// vested at time t.
    #[returns(::cosmwasm_std::Uint128)]
    Distributable {
        /// The time or none to use the current time.
        t: Option<Timestamp>,
    },
    /// Gets the current value of `vested(t)`. If `t` is `None`, the
    /// current time is used.
    #[returns(::cosmwasm_std::Uint128)]
    Vested { t: Option<Timestamp> },
    /// Gets the total amount that will ever vest, `max(vested(t))`.
    ///
    /// Note that if the contract is canceled at time c, this value
    /// will change to `vested(c)`. Thus, it can not be assumed to be
    /// constant over the contract's lifetime.
    #[returns(::cosmwasm_std::Uint128)]
    TotalToVest {},
    /// Gets the amount of time between the vest starting, and it
    /// completing. Returns `None` if the vest has been cancelled.
    #[returns(Option<::cosmwasm_std::Uint64>)]
    VestDuration {},
    /// Queries information about the contract's understanding of it's
    /// bonded and unbonding token balances. See the
    /// `StakeTrackerQuery` in `packages/cw-stake-tracker/lib.rs` for
    /// query methods and their return types.
    #[returns(::cosmwasm_std::Uint128)]
    Stake(StakeTrackerQuery),
}
