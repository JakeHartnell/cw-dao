use cw_orch::{interface, prelude::*};

use dao_voting_cw721_staked::contract::{execute, instantiate, migrate, query, reply};
use dao_voting_cw721_staked::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};

#[interface(InstantiateMsg, ExecuteMsg, QueryMsg, MigrateMsg)]
pub struct DaoVotingCw721Staked;

impl<Chain> Uploadable for DaoVotingCw721Staked<Chain> {
    /// Return the path to the wasm file corresponding to the contract
    fn wasm(_chain: &ChainInfoOwned) -> WasmPath {
        artifacts_dir_from_workspace!()
            .find_wasm_path("dao_voting_cw721_staked")
            .unwrap()
    }
    /// Returns a CosmWasm contract wrapper
    fn wrapper() -> Box<dyn MockContract<Empty>> {
        Box::new(
            ContractWrapper::new_with_empty(execute, instantiate, query)
                .with_reply(reply)
                .with_migrate(migrate),
        )
    }
}
