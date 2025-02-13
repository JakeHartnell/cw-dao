use cosmwasm_std::{Addr, Deps, DepsMut, Env, StdResult, Storage, Uint128};

use dao_voting::voting;

use crate::{
    state::{
        DAO, DELEGATED_VP, DELEGATES, PROPOSAL_HOOK_CALLERS, UNVOTED_DELEGATED_VP,
        VOTING_POWER_HOOK_CALLERS,
    },
    ContractError,
};

pub fn unregister_delegate(deps: DepsMut, delegate: &Addr, height: u64) -> StdResult<()> {
    DELEGATES.remove(deps.storage, delegate.clone(), height)
}

pub fn is_delegate_registered(deps: Deps, delegate: &Addr, height: Option<u64>) -> StdResult<bool> {
    let option = if let Some(height) = height {
        DELEGATES.may_load_at_height(deps.storage, delegate.clone(), height)
    } else {
        DELEGATES.may_load(deps.storage, delegate.clone())
    };

    option.map(|d| d.is_some())
}

pub fn get_voting_power(deps: Deps, addr: &Addr, height: u64) -> StdResult<Uint128> {
    let dao = DAO.load(deps.storage)?;
    voting::get_voting_power(deps, addr.clone(), &dao, Some(height))
}

/// Returns the unvoted delegated VP for a delegate on a proposal, falling back
/// to the delegate's total delegated VP at the given height if no unvoted
/// delegated VP exists for the proposal.
///
/// **NOTE: The caller is responsible for ensuring that the block height
/// corresponds to the correct height for the proposal.**
pub fn get_udvp(
    deps: Deps,
    delegate: &Addr,
    proposal_module: &Addr,
    proposal_id: u64,
    height: u64,
) -> StdResult<Uint128> {
    // if no unvoted delegated VP exists for the proposal, use the delegate's
    // total delegated VP at that height. UNVOTED_DELEGATED_VP gets set when one
    // of their delegators casts a vote. if empty, none of them have voted yet.
    match UNVOTED_DELEGATED_VP.may_load(deps.storage, (delegate, proposal_module, proposal_id))? {
        Some(vp) => Ok(vp),
        None => Ok(DELEGATED_VP
            .load(deps.storage, delegate.clone(), height)?
            .unwrap_or_default()),
    }
}

/// Ensures the delegation module is setup correctly.
pub fn ensure_setup(deps: Deps) -> Result<(), ContractError> {
    if VOTING_POWER_HOOK_CALLERS.is_empty(deps.storage)
        || PROPOSAL_HOOK_CALLERS.is_empty(deps.storage)
    {
        return Err(ContractError::DelegationModuleNotSetup {});
    }

    Ok(())
}

/// Add delegated VP from a delegator to a delegate, potentially with a given
/// expiration.
pub fn add_delegated_vp(
    storage: &mut dyn Storage,
    env: &Env,
    delegate: &Addr,
    vp: Uint128,
    expire_in: Option<u64>,
) -> StdResult<()> {
    DELEGATED_VP.increment(
        storage,
        delegate.clone(),
        // update at next block height to match 1-block delay behavior of voting
        // power queries and delegation changes. this matches the behavior of
        // creating a new delegation, which also starts on the following block.
        // if future delegations/undelegations/voting power changes occur in
        // this block, they will also load the state of the next block and
        // update the total that will be reflected in historical queries
        // starting from the next block.
        env.block.height + 1,
        vp,
    )?;

    // if expiration exists, decrement in the future at expiration height
    if let Some(expire_in) = expire_in {
        DELEGATED_VP.decrement(storage, delegate.clone(), env.block.height + expire_in, vp)?;
    }

    Ok(())
}

/// Remove delegated VP from a delegate, potentially with a given expiration.
pub fn remove_delegated_vp(
    storage: &mut dyn Storage,
    env: &Env,
    delegate: &Addr,
    vp: Uint128,
    original_expiration: Option<u64>,
) -> StdResult<()> {
    // if expiration was used when creating this delegation, first undo previous
    // decrement at end of expiration period. do this before undoing previous
    // increment to prevent underflow.
    if let Some(expiration) = original_expiration {
        DELEGATED_VP.increment(storage, delegate.clone(), expiration, vp)?;
    }

    DELEGATED_VP.decrement(
        storage,
        delegate.clone(),
        // update at next block height to match 1-block delay behavior of voting
        // power queries and delegation changes. this matches the behavior of
        // creating a new delegation, which also starts on the following block.
        // if future delegations/undelegations/voting power changes occur in
        // this block, they will also load the state of the next block and
        // update the total that will be reflected in historical queries
        // starting from the next block.
        env.block.height + 1,
        vp,
    )?;

    Ok(())
}
