use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    ensure, Addr, BlockInfo, Decimal, Deps, StdError, StdResult, Timestamp, Uint128, Uint256,
    Uint64,
};
use cw20::{Denom, Expiration};
use cw_storage_plus::{Item, Map};
use cw_utils::Duration;
use std::{cmp::min, collections::HashMap};

use crate::{
    helpers::{get_total_voting_power_at_block, scale_factor, DurationExt, ExpirationExt},
    rewards::get_active_total_earned_puvp,
    ContractError,
};

/// map user address to their unique reward state
pub const USER_REWARDS: Map<Addr, UserRewardState> = Map::new("ur");

/// map distribution ID to the its distribution state
pub const DISTRIBUTIONS: Map<u64, DistributionState> = Map::new("d");

/// map registered hooks to list of distribution IDs they're registered for
pub const REGISTERED_HOOKS: Map<Addr, Vec<u64>> = Map::new("rh");

/// The number of distributions that have been created.
pub const COUNT: Item<u64> = Item::new("count");

#[cw_serde]
#[derive(Default)]
pub struct UserRewardState {
    /// map distribution ID to the user's pending rewards that have been
    /// accounted for but not yet claimed.
    pub pending_rewards: HashMap<u64, Uint128>,
    /// map distribution ID to the user's earned rewards per unit voting power
    /// that have already been accounted for (added to pending and maybe
    /// claimed).
    pub accounted_for_rewards_puvp: HashMap<u64, Uint256>,
}

/// defines how many tokens (amount) should be distributed per amount of time
/// (duration). e.g. 5udenom per hour.
#[cw_serde]
pub enum EmissionRate {
    /// rewards are paused
    Paused {},
    /// rewards are distributed immediately
    Immediate {},
    /// rewards are distributed at a constant rate
    Linear {
        /// amount of tokens to distribute per amount of time
        amount: Uint128,
        /// duration of time to distribute amount
        duration: Duration,
        /// whether or not reward distribution is continuous: whether future
        /// funding after distribution finishes should be applied to the past,
        /// or rewards are paused once all funding has been distributed. all
        /// continuously backfilled rewards are distributed based on the current
        /// voting power.
        continuous: bool,
    },
}

impl EmissionRate {
    /// validate non-zero amount and duration if necessary
    pub fn validate(&self) -> Result<(), ContractError> {
        match self {
            EmissionRate::Paused {} => Ok(()),
            EmissionRate::Immediate {} => Ok(()),
            EmissionRate::Linear {
                amount, duration, ..
            } => {
                if amount.is_zero() {
                    return Err(ContractError::InvalidEmissionRateFieldZero {
                        field: "amount".to_string(),
                    });
                }
                if duration.is_zero() {
                    return Err(ContractError::InvalidEmissionRateFieldZero {
                        field: "duration".to_string(),
                    });
                }
                Ok(())
            }
        }
    }

    /// find the duration of the funded period given funded amount. e.g. if the
    /// funded amount is twice the emission rate amount, the funded period
    /// should be twice the emission rate duration, since the funded amount
    /// takes two emission cycles to be distributed.
    pub fn get_funded_period_duration(
        &self,
        funded_amount: Uint128,
    ) -> StdResult<Option<Duration>> {
        match self {
            // if rewards are paused, return no duration
            EmissionRate::Paused {} => Ok(None),
            // if rewards are immediate, return no duration
            EmissionRate::Immediate {} => Ok(None),
            // if rewards are linear, calculate based on funded amount
            EmissionRate::Linear {
                amount, duration, ..
            } => {
                let amount_to_emission_rate_ratio = Decimal::from_ratio(funded_amount, *amount);

                let funded_duration = match duration {
                    Duration::Height(h) => {
                        let duration_height = Uint128::from(*h)
                            .checked_mul_floor(amount_to_emission_rate_ratio)
                            .map_err(|e| StdError::generic_err(e.to_string()))?;
                        let duration = Uint64::try_from(duration_height)?.u64();
                        Duration::Height(duration)
                    }
                    Duration::Time(t) => {
                        let duration_time = Uint128::from(*t)
                            .checked_mul_floor(amount_to_emission_rate_ratio)
                            .map_err(|e| StdError::generic_err(e.to_string()))?;
                        let duration = Uint64::try_from(duration_time)?.u64();
                        Duration::Time(duration)
                    }
                };

                Ok(Some(funded_duration))
            }
        }
    }
}

#[cw_serde]
pub struct Epoch {
    /// reward emission rate
    pub emission_rate: EmissionRate,
    /// the time when the current reward distribution period started. period
    /// finishes iff it reaches its end.
    pub started_at: Expiration,
    /// the time when all funded rewards are allocated to users and thus the
    /// distribution period ends.
    pub ends_at: Expiration,
    /// total rewards earned per unit voting power from started_at to
    /// last_updated_total_earned_puvp
    pub total_earned_puvp: Uint256,
    /// time when total_earned_puvp was last updated
    pub last_updated_total_earned_puvp: Expiration,
}

impl Epoch {
    /// bump the last_updated_total_earned_puvp field to the minimum of the
    /// current block and ends_at since rewards cannot be distributed after
    /// ends_at. this is necessary in the case that a future funding backfills
    /// rewards after they've finished distributing. in order to compute over
    /// the missed space, last_updated can never be greater than ends_at. if
    /// ends_at is never, the epoch must be paused, so it should never be
    /// updated.
    pub fn bump_last_updated(&mut self, current_block: &BlockInfo) {
        match self.ends_at {
            Expiration::Never {} => {
                self.last_updated_total_earned_puvp = Expiration::Never {};
            }
            Expiration::AtHeight(ends_at_height) => {
                self.last_updated_total_earned_puvp =
                    Expiration::AtHeight(std::cmp::min(current_block.height, ends_at_height));
            }
            Expiration::AtTime(ends_at_time) => {
                self.last_updated_total_earned_puvp =
                    Expiration::AtTime(std::cmp::min(current_block.time, ends_at_time));
            }
        }
    }
}

/// the state of a reward distribution
#[cw_serde]
pub struct DistributionState {
    /// distribution ID
    pub id: u64,
    /// validated denom (native or cw20)
    pub denom: Denom,
    /// current distribution epoch state
    pub active_epoch: Epoch,
    /// address to query the voting power
    pub vp_contract: Addr,
    /// address that will update the reward split when the voting power
    /// distribution changes
    pub hook_caller: Addr,
    /// whether or not non-owners can fund the distribution
    pub open_funding: bool,
    /// total amount of rewards funded that will be distributed in the active
    /// epoch.
    pub funded_amount: Uint128,
    /// destination address for reward clawbacks
    pub withdraw_destination: Addr,
    /// historical rewards earned per unit voting power from past epochs due to
    /// changes in the emission rate. each time emission rate is changed, this
    /// value is increased by the `active_epoch`'s rewards earned puvp.
    pub historical_earned_puvp: Uint256,
}

impl DistributionState {
    pub fn get_denom_string(&self) -> String {
        match &self.denom {
            Denom::Native(denom) => denom.to_string(),
            Denom::Cw20(address) => address.to_string(),
        }
    }

    /// Returns the latest time when rewards were distributed. Works by
    /// comparing `current_block` with the distribution end time:
    /// - If the end is `Never`, then no rewards are currently being
    ///   distributed, so return the last update.
    /// - If the end is `AtHeight(h)` or `AtTime(t)`, we compare the current
    ///   block height or time with `h` or `t` respectively.
    /// - If current block respective value is before the end, rewards are still
    ///   being distributed. We therefore return the current block `height` or
    ///   `time`, as this block is the most recent time rewards were
    ///   distributed.
    /// - If current block respective value is after the end, rewards are no
    ///   longer being distributed. We therefore return the end `height` or
    ///   `time`, as that was the last date where rewards were distributed.
    pub fn get_latest_reward_distribution_time(&self, current_block: &BlockInfo) -> Expiration {
        match self.active_epoch.ends_at {
            Expiration::Never {} => self.active_epoch.last_updated_total_earned_puvp,
            Expiration::AtHeight(ends_at_height) => {
                Expiration::AtHeight(min(current_block.height, ends_at_height))
            }
            Expiration::AtTime(ends_at_time) => {
                Expiration::AtTime(min(current_block.time, ends_at_time))
            }
        }
    }

    /// get rewards to be distributed until the given expiration
    pub fn get_rewards_until(&self, expiration: Expiration) -> Result<Uint128, ContractError> {
        match self.active_epoch.emission_rate {
            EmissionRate::Paused {} => Ok(Uint128::zero()),
            EmissionRate::Immediate {} => Ok(self.funded_amount),
            EmissionRate::Linear {
                amount, duration, ..
            } => {
                // if not yet started, return 0.
                if let Expiration::Never {} = self.active_epoch.started_at {
                    return Ok(Uint128::zero());
                }

                let epoch_duration = expiration.duration_since(&self.active_epoch.started_at)?;

                // count total intervals of the rewards emission that will pass
                // based on the start and end times.
                let complete_distribution_periods = epoch_duration.ratio(&duration)?;

                Ok(amount.checked_mul_floor(complete_distribution_periods)?)
            }
        }
    }

    /// get the total rewards to be distributed based on the active epoch's
    /// emission rate and end time
    pub fn get_total_rewards(&self) -> Result<Uint128, ContractError> {
        self.get_rewards_until(self.active_epoch.ends_at)
    }

    /// get the currently undistributed rewards based on the active epoch's
    /// emission rate
    pub fn get_undistributed_rewards(
        &self,
        current_block: &BlockInfo,
    ) -> Result<Uint128, ContractError> {
        // get last time rewards were distributed (current block or previous end
        // time)
        let last_time_rewards_distributed = self.get_latest_reward_distribution_time(current_block);

        // get rewards distributed so far
        let distributed = self.get_rewards_until(last_time_rewards_distributed)?;

        // undistributed rewards are the remaining of the funded amount
        let undistributed = self.funded_amount.checked_sub(distributed)?;

        Ok(undistributed)
    }

    /// Finish current epoch early and start a new one with a new emission rate.
    pub fn transition_epoch(
        &mut self,
        deps: Deps,
        new_emission_rate: EmissionRate,
        current_block: &BlockInfo,
    ) -> Result<(), ContractError> {
        // if the new emission rate is the same as the active one, do nothing
        if self.active_epoch.emission_rate == new_emission_rate {
            return Ok(());
        }

        // 1. finish current epoch by updating rewards and setting end to the
        //    last time rewards were distributed (which is either the end date
        //    or the current block)
        self.active_epoch.total_earned_puvp =
            get_active_total_earned_puvp(deps, current_block, self)?;
        self.active_epoch.ends_at = self.get_latest_reward_distribution_time(current_block);

        // 2. add current epoch rewards earned to historical rewards
        self.historical_earned_puvp = self
            .historical_earned_puvp
            .checked_add(self.active_epoch.total_earned_puvp)
            .map_err(|err| ContractError::DistributionHistoryTooLarge {
                err: err.to_string(),
            })?;

        // 3. deduct the distributed rewards amount from total funded amount, as
        // those rewards are no longer distributed in the new epoch
        let active_epoch_earned_rewards = self.get_total_rewards()?;
        self.funded_amount = self
            .funded_amount
            .checked_sub(active_epoch_earned_rewards)?;

        // 4. start new epoch

        // we get the duration of the funded period and add it to the current
        // block height. if the sum overflows, we return u64::MAX, as it
        // suggests that the period is infinite or so long that it doesn't
        // matter.
        let new_ends_at = match new_emission_rate.get_funded_period_duration(self.funded_amount)? {
            Some(Duration::Height(h)) => {
                if current_block.height.checked_add(h).is_some() {
                    Expiration::AtHeight(current_block.height + h)
                } else {
                    Expiration::AtHeight(u64::MAX)
                }
            }
            Some(Duration::Time(t)) => {
                if current_block.time.seconds().checked_add(t).is_some() {
                    Expiration::AtTime(current_block.time.plus_seconds(t))
                } else {
                    Expiration::AtTime(Timestamp::from_seconds(u64::MAX))
                }
            }
            // if there is no funded period duration, but the emission rate is
            // immediate, set ends_at to the current block height to match
            // started_at below, since funds are distributed immediately
            None => Expiration::Never {},
        };

        let new_started_at = match new_emission_rate {
            EmissionRate::Paused {} => Expiration::Never {},
            EmissionRate::Immediate {} => Expiration::Never {},
            EmissionRate::Linear { duration, .. } => match duration {
                Duration::Height(_) => Expiration::AtHeight(current_block.height),
                Duration::Time(_) => Expiration::AtTime(current_block.time),
            },
        };

        self.active_epoch = Epoch {
            emission_rate: new_emission_rate.clone(),
            started_at: new_started_at,
            ends_at: new_ends_at,
            // start the new active epoch with zero rewards earned
            total_earned_puvp: Uint256::zero(),
            last_updated_total_earned_puvp: new_started_at,
        };

        // if new emission rate is immediate, update total_earned_puvp with
        // remaining funded_amount right away
        if (self.active_epoch.emission_rate == EmissionRate::Immediate {}) {
            self.update_immediate_emission_total_earned_puvp(
                deps,
                current_block,
                self.funded_amount,
            )?;
        }

        Ok(())
    }

    /// Update the total_earned_puvp field in the active epoch for immediate
    /// emission. This logic normally lives in get_active_total_earned_puvp, but
    /// we need only need to execute this right when funding, and we need to
    /// know the delta in funded amount, which is not accessible anywhere other
    /// than when being funded or transitioning to a new emission rate.
    pub fn update_immediate_emission_total_earned_puvp(
        &mut self,
        deps: Deps,
        block: &BlockInfo,
        funded_amount_delta: Uint128,
    ) -> Result<(), ContractError> {
        // should never happen
        ensure!(
            self.active_epoch.emission_rate == EmissionRate::Immediate {},
            ContractError::Std(StdError::generic_err(format!(
                "expected immediate emission, got {:?}",
                self.active_epoch.emission_rate
            )))
        );

        let curr = self.active_epoch.total_earned_puvp;

        let total_power = get_total_voting_power_at_block(deps, block, &self.vp_contract)?;

        // if no voting power is registered, error since rewards can't be
        // distributed.
        if total_power.is_zero() {
            Err(ContractError::NoVotingPowerNoRewards {})
        } else {
            // the new rewards per unit voting power based on the funded amount
            let new_rewards_puvp = Uint256::from(funded_amount_delta)
                // this can never overflow since funded_amount is a Uint128
                .checked_mul(scale_factor())?
                .checked_div(total_power.into())?;

            self.active_epoch.total_earned_puvp = curr.checked_add(new_rewards_puvp)?;

            Ok(())
        }
    }
}
