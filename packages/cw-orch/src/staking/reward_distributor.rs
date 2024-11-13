use cw_orch::{interface, prelude::*};

use cw20_stake_reward_distributor::contract::{execute, instantiate, migrate, query};
use cw20_stake_reward_distributor::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};

#[interface(InstantiateMsg, ExecuteMsg, QueryMsg, MigrateMsg)]
pub struct DaoStakingCw20RewardDistributor;

impl<Chain> Uploadable for DaoStakingCw20RewardDistributor<Chain> {
    /// Return the path to the wasm file corresponding to the contract
    fn wasm(_chain: &ChainInfoOwned) -> WasmPath {
        artifacts_dir_from_workspace!()
            .find_wasm_path("cw20_stake_reward_distributor")
            .unwrap()
    }
    /// Returns a CosmWasm contract wrapper
    fn wrapper() -> Box<dyn MockContract<Empty>> {
        Box::new(ContractWrapper::new_with_empty(execute, instantiate, query).with_migrate(migrate))
    }
}
