use cosmwasm_std::Empty;
use cw_multi_test::{Contract, ContractWrapper};

pub fn dao_dao_core_v241_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        dao_dao_core_v241::contract::execute,
        dao_dao_core_v241::contract::instantiate,
        dao_dao_core_v241::contract::query,
    )
    .with_reply(dao_dao_core_v241::contract::reply)
    .with_migrate(dao_dao_core_v241::contract::migrate);
    Box::new(contract)
}

pub fn dao_voting_cw4_v241_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        dao_voting_cw4_v241::contract::execute,
        dao_voting_cw4_v241::contract::instantiate,
        dao_voting_cw4_v241::contract::query,
    )
    .with_reply(dao_voting_cw4_v241::contract::reply)
    .with_migrate(dao_voting_cw4_v241::contract::migrate);
    Box::new(contract)
}

pub fn dao_proposal_single_v241_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        dao_proposal_single_v241::contract::execute,
        dao_proposal_single_v241::contract::instantiate,
        dao_proposal_single_v241::contract::query,
    )
    .with_reply(dao_proposal_single_v241::contract::reply)
    .with_migrate(dao_proposal_single_v241::contract::migrate);
    Box::new(contract)
}

pub fn dao_proposal_multiple_v241_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        dao_proposal_multiple_v241::contract::execute,
        dao_proposal_multiple_v241::contract::instantiate,
        dao_proposal_multiple_v241::contract::query,
    )
    .with_reply(dao_proposal_multiple_v241::contract::reply)
    .with_migrate(dao_proposal_multiple_v241::contract::migrate);
    Box::new(contract)
}

pub fn dao_pre_propose_single_v241_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        dao_pre_propose_single_v241::contract::execute,
        dao_pre_propose_single_v241::contract::instantiate,
        dao_pre_propose_single_v241::contract::query,
    );
    Box::new(contract)
}

pub fn dao_pre_propose_approval_single_v241_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        dao_pre_propose_approval_single_v241::contract::execute,
        dao_pre_propose_approval_single_v241::contract::instantiate,
        dao_pre_propose_approval_single_v241::contract::query,
    );
    Box::new(contract)
}

pub fn dao_pre_propose_multiple_v241_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        dao_pre_propose_multiple_v241::contract::execute,
        dao_pre_propose_multiple_v241::contract::instantiate,
        dao_pre_propose_multiple_v241::contract::query,
    );
    Box::new(contract)
}
