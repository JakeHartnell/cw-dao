use cosmwasm_schema::cw_serde;
use cosmwasm_std::Addr;
use cw_storage_plus::{Index, IndexList, IndexedMap, Item, MultiIndex};

/// Temporarily holds the address of the instantiator for use in submessages
pub const TMP_INSTANTIATOR_INFO: Item<Addr> = Item::new("tmp_instantiator_info");
pub const VESTING_CODE_ID: Item<u64> = Item::new("pci");

#[cw_serde]
pub struct VestingContract {
    pub contract: String,
    pub instantiator: String,
    pub recipient: String,
}

pub struct TokenIndexes<'a> {
    pub instantiator: MultiIndex<'a, String, VestingContract, String>,
    pub recipient: MultiIndex<'a, String, VestingContract, String>,
}

impl IndexList<VestingContract> for TokenIndexes<'_> {
    fn get_indexes(&self) -> Box<dyn Iterator<Item = &dyn Index<VestingContract>> + '_> {
        let v: Vec<&dyn Index<VestingContract>> = vec![&self.instantiator, &self.recipient];
        Box::new(v.into_iter())
    }
}

pub fn vesting_contracts<'a>() -> IndexedMap<'a, &'a str, VestingContract, TokenIndexes<'a>> {
    let indexes = TokenIndexes {
        instantiator: MultiIndex::new(
            |_pk: &[u8], d: &VestingContract| d.instantiator.clone(),
            "vesting_contracts",
            "vesting_contracts__instantiator",
        ),
        recipient: MultiIndex::new(
            |_pk: &[u8], d: &VestingContract| d.recipient.clone(),
            "vesting_contracts",
            "vesting_contracts__recipient",
        ),
    };
    IndexedMap::new("vesting_contracts", indexes)
}
