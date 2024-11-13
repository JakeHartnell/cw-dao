use cw_orch::{interface, prelude::*};

use dao_voting_cw20_balance::contract::{execute, instantiate, query, reply};
use dao_voting_cw20_balance::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};

#[interface(InstantiateMsg, ExecuteMsg, QueryMsg, Empty)]
pub struct DaoVotingCw20Balance;

impl<Chain> Uploadable for DaoVotingCw20Balance<Chain> {
    /// Return the path to the wasm file corresponding to the contract
    fn wasm(_chain: &ChainInfoOwned) -> WasmPath {
        artifacts_dir_from_workspace!()
            .find_wasm_path("dao_voting_cw20_balance")
            .unwrap()
    }
    /// Returns a CosmWasm contract wrapper
    fn wrapper() -> Box<dyn MockContract<Empty>> {
        Box::new(ContractWrapper::new_with_empty(execute, instantiate, query).with_reply(reply))
    }
}
