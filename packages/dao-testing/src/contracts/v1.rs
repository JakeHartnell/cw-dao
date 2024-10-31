use cosmwasm_std::Empty;
use cw_multi_test::{Contract, ContractWrapper};

pub fn cw_proposal_single_v1_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw_proposal_single_v1::contract::execute,
        cw_proposal_single_v1::contract::instantiate,
        cw_proposal_single_v1::contract::query,
    )
    .with_reply(cw_proposal_single_v1::contract::reply)
    .with_migrate(cw_proposal_single_v1::contract::migrate);
    Box::new(contract)
}

pub fn cw_core_v1_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw_core_v1::contract::execute,
        cw_core_v1::contract::instantiate,
        cw_core_v1::contract::query,
    )
    .with_reply(cw_core_v1::contract::reply)
    .with_migrate(cw_core_v1::contract::migrate);
    Box::new(contract)
}

pub fn cw4_voting_v1_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw4_voting_v1::contract::execute,
        cw4_voting_v1::contract::instantiate,
        cw4_voting_v1::contract::query,
    )
    .with_reply(cw4_voting_v1::contract::reply)
    .with_migrate(cw4_voting_v1::contract::migrate);
    Box::new(contract)
}

pub fn cw20_stake_v1_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw20_stake_v1::contract::execute,
        cw20_stake_v1::contract::instantiate,
        cw20_stake_v1::contract::query,
    )
    .with_migrate(cw20_stake_v1::contract::migrate);
    Box::new(contract)
}

pub fn stake_cw20_v03_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        stake_cw20_v03::contract::execute,
        stake_cw20_v03::contract::instantiate,
        stake_cw20_v03::contract::query,
    );
    Box::new(contract)
}

pub fn cw20_stake_external_rewards_v1_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw20_stake_external_rewards_v1::contract::execute,
        cw20_stake_external_rewards_v1::contract::instantiate,
        cw20_stake_external_rewards_v1::contract::query,
    )
    .with_migrate(cw20_stake_external_rewards_v1::contract::migrate);
    Box::new(contract)
}

pub fn cw20_stake_reward_distributor_v1_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw20_stake_reward_distributor_v1::contract::execute,
        cw20_stake_reward_distributor_v1::contract::instantiate,
        cw20_stake_reward_distributor_v1::contract::query,
    );
    Box::new(contract)
}
