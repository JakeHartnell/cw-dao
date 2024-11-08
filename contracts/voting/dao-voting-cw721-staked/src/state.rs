use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Binary, Empty, StdError, StdResult, Storage, Uint128};
use cw_hooks::Hooks;
use cw_storage_plus::{Item, Map, SnapshotItem, SnapshotMap, Strategy};
use cw_utils::Duration;
use dao_voting::threshold::ActiveThreshold;
use nft_controllers::NftClaims;

use crate::ContractError;

#[cw_serde]
pub struct Config {
    pub nft_address: Addr,
    pub unstaking_duration: Option<Duration>,
}

pub const ACTIVE_THRESHOLD: Item<ActiveThreshold> = Item::new("active_threshold");
pub const CONFIG: Item<Config> = Item::new("config");
pub const DAO: Item<Addr> = Item::new("dao");

// Holds initial NFTs messages during instantiation.
pub const INITIAL_NFTS: Item<Vec<Binary>> = Item::new("initial_nfts");

/// The set of NFTs currently staked by each address. The existence of
/// an `(address, token_id)` pair implies that `address` has staked
/// `token_id`.
pub const STAKED_NFTS_PER_OWNER: Map<(&Addr, &str), Empty> = Map::new("snpw");
/// The number of NFTs staked by an address as a function of block
/// height.
pub const NFT_BALANCES: SnapshotMap<&Addr, Uint128> = SnapshotMap::new(
    "nb",
    "nb__checkpoints",
    "nb__changelog",
    Strategy::EveryBlock,
);
/// The number of NFTs staked with this contract as a function of
/// block height.
pub const TOTAL_STAKED_NFTS: SnapshotItem<Uint128> = SnapshotItem::new(
    "tsn",
    "tsn__checkpoints",
    "tsn__changelog",
    Strategy::EveryBlock,
);

/// The legacy NFT claims storage uses a non-paginatable vector, which limits
/// the number of claims that may be outstanding. This is horrible UX,
/// especially for large NFT collections. To allow DAOs to upgrade, we must keep
/// the legacy NFT claims storage, but we can paginate the new storage.
pub const LEGACY_NFT_CLAIMS: cw721_controllers_v250::NftClaims =
    cw721_controllers_v250::NftClaims::new("nft_claims");

pub const NFT_CLAIMS: NftClaims = NftClaims::new("nc");

// Hooks to contracts that will receive staking and unstaking
// messages.
pub const HOOKS: Hooks = Hooks::new("hooks");

pub fn register_staked_nft(
    storage: &mut dyn Storage,
    height: u64,
    staker: &Addr,
    token_id: &String,
) -> StdResult<()> {
    let add_one = |prev: Option<Uint128>| -> StdResult<Uint128> {
        prev.unwrap_or_default()
            .checked_add(Uint128::new(1))
            .map_err(StdError::overflow)
    };

    STAKED_NFTS_PER_OWNER.save(storage, (staker, token_id), &Empty::default())?;
    NFT_BALANCES.update(storage, staker, height, add_one)?;
    TOTAL_STAKED_NFTS
        .update(storage, height, add_one)
        .map(|_| ())
}

/// Registers the unstaking of TOKEN_IDs in storage. Errors if:
///
/// 1. `token_ids` is non-unique.
/// 2. a NFT being staked has not previously been staked.
pub fn register_unstaked_nfts(
    storage: &mut dyn Storage,
    height: u64,
    staker: &Addr,
    token_ids: &[String],
) -> Result<(), ContractError> {
    let subtractor = |amount: u128| {
        move |prev: Option<Uint128>| -> StdResult<Uint128> {
            prev.expect("unstaking that which was not staked")
                .checked_sub(Uint128::new(amount))
                .map_err(StdError::overflow)
        }
    };

    for token in token_ids {
        let key = (staker, token.as_str());
        if STAKED_NFTS_PER_OWNER.has(storage, key) {
            STAKED_NFTS_PER_OWNER.remove(storage, key);
        } else {
            return Err(ContractError::NotStaked {
                token_id: token.clone(),
            });
        }
    }

    // invariant: token_ids has unique values. for loop asserts this.

    let sub_n = subtractor(token_ids.len() as u128);
    TOTAL_STAKED_NFTS.update(storage, height, sub_n)?;
    NFT_BALANCES.update(storage, staker, height, sub_n)?;
    Ok(())
}
