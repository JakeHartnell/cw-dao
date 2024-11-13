use cw_orch::{interface, prelude::*};

use dao_proposal_condorcet::contract::{execute, instantiate, query, reply};
use dao_proposal_condorcet::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};

#[interface(InstantiateMsg, ExecuteMsg, QueryMsg, Empty)]
pub struct DaoProposalCondorcet;

impl<Chain> Uploadable for DaoProposalCondorcet<Chain> {
    /// Return the path to the wasm file corresponding to the contract
    fn wasm(_chain: &ChainInfoOwned) -> WasmPath {
        artifacts_dir_from_workspace!()
            .find_wasm_path("dao_proposal_condorcet")
            .unwrap()
    }
    /// Returns a CosmWasm contract wrapper
    fn wrapper() -> Box<dyn MockContract<Empty>> {
        Box::new(ContractWrapper::new_with_empty(execute, instantiate, query).with_reply(reply))
    }
}
