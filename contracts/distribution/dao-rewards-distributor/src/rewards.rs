use cosmwasm_std::{Addr, BlockInfo, Deps, DepsMut, Env, StdResult, Uint128, Uint256};
use cw20::Expiration;

use crate::{
    helpers::{
        get_total_voting_power_at_block, get_voting_power_at_block, scale_factor, DurationExt,
        ExpirationExt,
    },
    state::{DistributionState, EmissionRate, UserRewardState, DISTRIBUTIONS, USER_REWARDS},
    ContractError,
};

/// updates the user reward state for a given distribution and user address.
/// also syncs the global reward state with the latest puvp values.
pub fn update_rewards(
    deps: &mut DepsMut,
    env: &Env,
    addr: &Addr,
    distribution_id: u64,
) -> Result<(), ContractError> {
    let mut distribution = DISTRIBUTIONS
        .load(deps.storage, distribution_id)
        .map_err(|_| ContractError::DistributionNotFound {
            id: distribution_id,
        })?;

    // user may not have a reward state set yet if that is their first time
    // claiming, so we default to an empty state
    let mut user_reward_state = USER_REWARDS
        .may_load(deps.storage, addr.clone())?
        .unwrap_or_default();

    // first update the active epoch earned puvp value up to the current block
    distribution.active_epoch.total_earned_puvp =
        get_active_total_earned_puvp(deps.as_ref(), &env.block, &distribution)?;
    distribution.active_epoch.bump_last_updated(&env.block);

    // then calculate the total applicable puvp, which is the sum of historical
    // rewards earned puvp and the active epoch total earned puvp we just
    // updated above based on the current block
    let total_applicable_puvp = distribution
        .active_epoch
        .total_earned_puvp
        .checked_add(distribution.historical_earned_puvp)?;

    let unaccounted_for_rewards = get_accrued_rewards_not_yet_accounted_for(
        deps.as_ref(),
        env,
        addr,
        total_applicable_puvp,
        &distribution,
        &user_reward_state,
    )?;

    // get the pre-existing pending reward amount for the distribution
    let previous_pending_reward_amount = user_reward_state
        .pending_rewards
        .get(&distribution.id)
        .cloned()
        .unwrap_or_default();

    let amount_sum = unaccounted_for_rewards.checked_add(previous_pending_reward_amount)?;

    // get the amount of newly earned rewards for the distribution
    user_reward_state
        .pending_rewards
        .insert(distribution_id, amount_sum);

    // update the accounted for amount to that of the total applicable puvp
    user_reward_state
        .accounted_for_rewards_puvp
        .insert(distribution_id, total_applicable_puvp);

    // reflect the updated state changes
    USER_REWARDS.save(deps.storage, addr.clone(), &user_reward_state)?;
    DISTRIBUTIONS.save(deps.storage, distribution_id, &distribution)?;

    Ok(())
}

/// Calculate the total rewards per unit voting power in the active epoch.
pub fn get_active_total_earned_puvp(
    deps: Deps,
    block: &BlockInfo,
    distribution: &DistributionState,
) -> Result<Uint256, ContractError> {
    match distribution.active_epoch.emission_rate {
        EmissionRate::Paused {} => Ok(Uint256::zero()),
        // this is updated manually during funding, so just return it here.
        EmissionRate::Immediate {} => Ok(distribution.active_epoch.total_earned_puvp),
        EmissionRate::Linear {
            amount, duration, ..
        } => {
            let curr = distribution.active_epoch.total_earned_puvp;

            let last_time_rewards_distributed =
                distribution.get_latest_reward_distribution_time(block);

            // if never distributed rewards (i.e. not yet funded), return
            // current, which must be 0.
            if let Expiration::Never {} = last_time_rewards_distributed {
                return Ok(curr);
            }

            // get the duration from the last time rewards were updated to the
            // last time rewards were distributed. this will be 0 if the rewards
            // were updated at or after the last time rewards were distributed.
            let new_reward_distribution_duration = last_time_rewards_distributed
                .duration_since(&distribution.active_epoch.last_updated_total_earned_puvp)?;

            // no need to query total voting power and do math if distribution
            // is already up to date.
            if new_reward_distribution_duration.is_zero() {
                return Ok(curr);
            }

            let total_power =
                get_total_voting_power_at_block(deps, block, &distribution.vp_contract)?;

            // if no voting power is registered, no one should receive rewards.
            if total_power.is_zero() {
                Ok(curr)
            } else {
                // count (partial) intervals of the rewards emission that have
                // passed since the last update which need to be distributed
                let complete_distribution_periods =
                    new_reward_distribution_duration.ratio(&duration)?;

                let new_rewards_distributed = Uint256::from(amount)
                    .checked_mul_floor(complete_distribution_periods)?
                    .checked_mul(scale_factor())?;

                // the new rewards per unit voting power that have been
                // distributed since the last update
                let new_rewards_puvp = new_rewards_distributed.checked_div(total_power.into())?;
                Ok(curr.checked_add(new_rewards_puvp)?)
            }
        }
    }
}

// get a user's rewards not yet accounted for in their reward state (not pending
// nor claimed, but available to them due to the passage of time).
pub fn get_accrued_rewards_not_yet_accounted_for(
    deps: Deps,
    env: &Env,
    addr: &Addr,
    total_earned_puvp: Uint256,
    distribution: &DistributionState,
    user_reward_state: &UserRewardState,
) -> StdResult<Uint128> {
    // get the user's voting power at the current height
    let voting_power: Uint256 =
        get_voting_power_at_block(deps, &env.block, &distribution.vp_contract, addr)?.into();

    // get previous reward per unit voting power accounted for
    let user_last_reward_puvp = user_reward_state
        .accounted_for_rewards_puvp
        .get(&distribution.id)
        .cloned()
        .unwrap_or_default();

    // calculate the difference between the current total reward per unit
    // voting power distributed and the user's latest reward per unit voting
    // power accounted for.
    let reward_factor = total_earned_puvp.checked_sub(user_last_reward_puvp)?;

    // calculate the amount of rewards earned:
    // voting_power * reward_factor / scale_factor
    let accrued_rewards_amount: Uint128 = voting_power
        .checked_mul(reward_factor)?
        .checked_div(scale_factor())?
        .try_into()?;

    Ok(accrued_rewards_amount)
}
