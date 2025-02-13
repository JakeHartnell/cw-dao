use cosmwasm_std::{Addr, Decimal, Uint128};
use cw_snapshot_vector_map::SnapshotVectorMap;
use cw_storage_plus::{Item, Map, SnapshotItem, SnapshotMap, Strategy};

use cw_wormhole::Wormhole;

// make these types directly available to consumers of this crate
pub use dao_voting::delegation::{Config, Delegate, Delegation};

/// the configuration of the delegation system.
pub const CONFIG: Item<Config> = Item::new("config");

/// the maximum percent of voting power that a single delegate can wield. they
/// can be delegated any amount of voting power—this cap is only applied when
/// casting votes. historical values must be stored so that proposals that
/// already exist use deterministic math.
///
/// this is separate from config since we need historical queries for it.
pub const VP_CAP_PERCENT: SnapshotItem<Option<Decimal>> = SnapshotItem::new(
    "vpc",
    "vpc__checkpoints",
    "vpc__changelog",
    Strategy::EveryBlock,
);

/// the DAO this delegation system is connected to.
pub const DAO: Item<Addr> = Item::new("dao");

/// the active proposal modules loaded from the DAO that can execute
/// proposal-related hooks.
pub const PROPOSAL_HOOK_CALLERS: Map<Addr, ()> = Map::new("phc");

/// the contracts that can execute the voting power change hooks. these should
/// be DAO voting modules or their associated staking contracts.
pub const VOTING_POWER_HOOK_CALLERS: Map<Addr, ()> = Map::new("vphc");

/// the delegates.
pub const DELEGATES: SnapshotMap<Addr, Delegate> = SnapshotMap::new(
    "delegates",
    "delegates__checkpoints",
    "delegates__changelog",
    Strategy::EveryBlock,
);

/// map (delegate, proposal_module, proposal_id) -> the VP delegated to the
/// delegate that has not yet been used in votes cast by delegators in a
/// specific proposal.
pub const UNVOTED_DELEGATED_VP: Map<(&Addr, &Addr, u64), Uint128> = Map::new("udvp");

/// the VP delegated to a delegate by height. Wormhole allows us to update
/// delegated VP in the future, which we need for implementing automatic
/// delegation expiration.
pub const DELEGATED_VP: Wormhole<Addr, Uint128> = Wormhole::new("dvp");

/// the delegations of a delegator.
pub const DELEGATIONS: SnapshotVectorMap<Addr, Delegation> = SnapshotVectorMap::new(
    "d__items",
    "d__next_ids",
    "d__active",
    "d__active__checkpoints",
    "d__active__changelog",
    "d__active__last_update",
);

/// map (delegator, delegate) -> (ID, expiration_block) of the delegation in the
/// vector map. this is useful for quickly checking if a delegation already
/// exists, and for undelegating.
pub const DELEGATION_ENTRIES: Map<(&Addr, &Addr), (u64, Option<u64>)> = Map::new("dids");

/// map delegator -> percent delegated to all delegates.
pub const PERCENT_DELEGATED: Map<&Addr, Decimal> = Map::new("pd");
