use cosmwasm_std::Empty;
use cw_multi_test::{Contract, ContractWrapper};

pub fn cw20_base_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw20_base::contract::execute,
        cw20_base::contract::instantiate,
        cw20_base::contract::query,
    )
    .with_migrate(cw20_base::contract::migrate);
    Box::new(contract)
}

pub fn cw4_group_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw4_group::contract::execute,
        cw4_group::contract::instantiate,
        cw4_group::contract::query,
    );
    Box::new(contract)
}

pub fn cw721_base_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw721_base::entry::execute,
        cw721_base::entry::instantiate,
        cw721_base::entry::query,
    )
    .with_migrate(cw721_base::entry::migrate);
    Box::new(contract)
}

pub fn cw721_roles_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw721_roles::contract::execute,
        cw721_roles::contract::instantiate,
        cw721_roles::contract::query,
    );
    Box::new(contract)
}

pub fn cw20_stake_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw20_stake::contract::execute,
        cw20_stake::contract::instantiate,
        cw20_stake::contract::query,
    )
    .with_migrate(cw20_stake::contract::migrate);
    Box::new(contract)
}

pub fn dao_proposal_condorcet_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        dao_proposal_condorcet::contract::execute,
        dao_proposal_condorcet::contract::instantiate,
        dao_proposal_condorcet::contract::query,
    )
    .with_reply(dao_proposal_condorcet::contract::reply);
    Box::new(contract)
}

pub fn dao_proposal_single_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        dao_proposal_single::contract::execute,
        dao_proposal_single::contract::instantiate,
        dao_proposal_single::contract::query,
    )
    .with_reply(dao_proposal_single::contract::reply)
    .with_migrate(dao_proposal_single::contract::migrate);
    Box::new(contract)
}

pub fn dao_proposal_multiple_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        dao_proposal_multiple::contract::execute,
        dao_proposal_multiple::contract::instantiate,
        dao_proposal_multiple::contract::query,
    )
    .with_reply(dao_proposal_multiple::contract::reply)
    .with_migrate(dao_proposal_multiple::contract::migrate);
    Box::new(contract)
}

pub fn dao_proposal_sudo_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        dao_proposal_sudo::contract::execute,
        dao_proposal_sudo::contract::instantiate,
        dao_proposal_sudo::contract::query,
    );
    Box::new(contract)
}

pub fn dao_pre_propose_approver_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        dao_pre_propose_approver::contract::execute,
        dao_pre_propose_approver::contract::instantiate,
        dao_pre_propose_approver::contract::query,
    )
    .with_migrate(dao_pre_propose_approver::contract::migrate);
    Box::new(contract)
}

pub fn dao_pre_propose_single_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        dao_pre_propose_single::contract::execute,
        dao_pre_propose_single::contract::instantiate,
        dao_pre_propose_single::contract::query,
    )
    .with_migrate(dao_pre_propose_single::contract::migrate);
    Box::new(contract)
}

pub fn dao_pre_propose_multiple_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        dao_pre_propose_multiple::contract::execute,
        dao_pre_propose_multiple::contract::instantiate,
        dao_pre_propose_multiple::contract::query,
    )
    .with_migrate(dao_pre_propose_multiple::contract::migrate);
    Box::new(contract)
}

pub fn dao_pre_propose_approval_single_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        dao_pre_propose_approval_single::contract::execute,
        dao_pre_propose_approval_single::contract::instantiate,
        dao_pre_propose_approval_single::contract::query,
    )
    .with_migrate(dao_pre_propose_approval_single::contract::migrate);
    Box::new(contract)
}

pub fn dao_voting_cw4_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        dao_voting_cw4::contract::execute,
        dao_voting_cw4::contract::instantiate,
        dao_voting_cw4::contract::query,
    )
    .with_reply(dao_voting_cw4::contract::reply)
    .with_migrate(dao_voting_cw4::contract::migrate);
    Box::new(contract)
}

pub fn dao_voting_cw20_staked_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        dao_voting_cw20_staked::contract::execute,
        dao_voting_cw20_staked::contract::instantiate,
        dao_voting_cw20_staked::contract::query,
    )
    .with_reply(dao_voting_cw20_staked::contract::reply)
    .with_migrate(dao_voting_cw20_staked::contract::migrate);
    Box::new(contract)
}

pub fn dao_voting_cw20_balance_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        dao_voting_cw20_balance::contract::execute,
        dao_voting_cw20_balance::contract::instantiate,
        dao_voting_cw20_balance::contract::query,
    )
    .with_reply(dao_voting_cw20_balance::contract::reply);
    Box::new(contract)
}

pub fn dao_voting_token_staked_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        dao_voting_token_staked::contract::execute,
        dao_voting_token_staked::contract::instantiate,
        dao_voting_token_staked::contract::query,
    )
    .with_reply(dao_voting_token_staked::contract::reply)
    .with_migrate(dao_voting_token_staked::contract::migrate);
    Box::new(contract)
}

pub fn dao_voting_cw721_staked_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        dao_voting_cw721_staked::contract::execute,
        dao_voting_cw721_staked::contract::instantiate,
        dao_voting_cw721_staked::contract::query,
    )
    .with_reply(dao_voting_cw721_staked::contract::reply)
    .with_migrate(dao_voting_cw721_staked::contract::migrate);
    Box::new(contract)
}

pub fn dao_voting_cw721_roles_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        dao_voting_cw721_roles::contract::execute,
        dao_voting_cw721_roles::contract::instantiate,
        dao_voting_cw721_roles::contract::query,
    )
    .with_reply(dao_voting_cw721_roles::contract::reply);
    Box::new(contract)
}

pub fn dao_voting_onft_staked_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        dao_voting_onft_staked::contract::execute,
        dao_voting_onft_staked::contract::instantiate,
        dao_voting_onft_staked::contract::query,
    )
    .with_migrate(dao_voting_onft_staked::contract::migrate);
    Box::new(contract)
}

pub fn dao_dao_core_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        dao_dao_core::contract::execute,
        dao_dao_core::contract::instantiate,
        dao_dao_core::contract::query,
    )
    .with_reply(dao_dao_core::contract::reply)
    .with_migrate(dao_dao_core::contract::migrate);
    Box::new(contract)
}

pub fn dao_migrator_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        dao_migrator::contract::execute,
        dao_migrator::contract::instantiate,
        dao_migrator::contract::query,
    )
    .with_reply(dao_migrator::contract::reply);
    Box::new(contract)
}

pub fn cw_vesting_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw_vesting::contract::execute,
        cw_vesting::contract::instantiate,
        cw_vesting::contract::query,
    );
    Box::new(contract)
}

pub fn dao_test_custom_factory_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        dao_test_custom_factory::contract::execute,
        dao_test_custom_factory::contract::instantiate,
        dao_test_custom_factory::contract::query,
    )
    .with_reply(dao_test_custom_factory::contract::reply);
    Box::new(contract)
}

pub fn cw_fund_distributor_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw_fund_distributor::contract::execute,
        cw_fund_distributor::contract::instantiate,
        cw_fund_distributor::contract::query,
    )
    .with_migrate(cw_fund_distributor::contract::migrate);
    Box::new(contract)
}

pub fn dao_rewards_distributor_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        dao_rewards_distributor::contract::execute,
        dao_rewards_distributor::contract::instantiate,
        dao_rewards_distributor::contract::query,
    )
    .with_migrate(dao_rewards_distributor::contract::migrate);
    Box::new(contract)
}

pub fn btsg_ft_factory_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        btsg_ft_factory::contract::execute,
        btsg_ft_factory::contract::instantiate,
        btsg_ft_factory::contract::query,
    )
    .with_reply(btsg_ft_factory::contract::reply)
    .with_migrate(btsg_ft_factory::contract::migrate);
    Box::new(contract)
}

pub fn cw_admin_factory_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw_admin_factory::contract::execute,
        cw_admin_factory::contract::instantiate,
        cw_admin_factory::contract::query,
    )
    .with_reply(cw_admin_factory::contract::reply)
    .with_migrate(cw_admin_factory::contract::migrate);
    Box::new(contract)
}

pub fn cw_payroll_factory_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw_payroll_factory::contract::execute,
        cw_payroll_factory::contract::instantiate,
        cw_payroll_factory::contract::query,
    )
    .with_reply(cw_payroll_factory::contract::reply);
    Box::new(contract)
}

pub fn cw_token_swap_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw_token_swap::contract::execute,
        cw_token_swap::contract::instantiate,
        cw_token_swap::contract::query,
    )
    .with_migrate(cw_token_swap::contract::migrate);
    Box::new(contract)
}

pub fn cw20_stake_external_rewards_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw20_stake_external_rewards::contract::execute,
        cw20_stake_external_rewards::contract::instantiate,
        cw20_stake_external_rewards::contract::query,
    )
    .with_migrate(cw20_stake_external_rewards::contract::migrate);
    Box::new(contract)
}

pub fn cw20_stake_reward_distributor_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw20_stake_reward_distributor::contract::execute,
        cw20_stake_reward_distributor::contract::instantiate,
        cw20_stake_reward_distributor::contract::query,
    )
    .with_migrate(cw20_stake_reward_distributor::contract::migrate);
    Box::new(contract)
}

pub fn dao_proposal_hook_counter_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        dao_proposal_hook_counter::contract::execute,
        dao_proposal_hook_counter::contract::instantiate,
        dao_proposal_hook_counter::contract::query,
    );
    Box::new(contract)
}
