use cosmwasm_std::Empty;
use cw_multi_test::{Contract, ContractWrapper};

mod adversarial_tests;
mod tests;

pub fn cw_fund_distributor_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        crate::contract::execute,
        crate::contract::instantiate,
        crate::contract::query,
    )
    .with_migrate(crate::contract::migrate);
    Box::new(contract)
}
