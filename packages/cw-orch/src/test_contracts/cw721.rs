use cosmwasm_std::Empty;
use cw721_base::{
    entry::{execute, instantiate, query},
    ExecuteMsg, InstantiateMsg, QueryMsg,
};
use cw_orch::{interface, prelude::*};

pub type Cw721BaseQueryMsg = QueryMsg<Empty>;
#[interface(InstantiateMsg, ExecuteMsg<T, E>, Cw721BaseQueryMsg, Empty)]
pub struct Cw721BaseGeneric;

impl<Chain: CwEnv, T, E> Uploadable for Cw721BaseGeneric<Chain, T, E> {
    // Return a CosmWasm contract wrapper
    fn wrapper() -> Box<dyn MockContract<Empty>> {
        Box::new(ContractWrapper::new_with_empty(execute, instantiate, query))
    }
}

pub type Cw721Base<Chain> = Cw721BaseGeneric<Chain, Option<Empty>, Empty>;
