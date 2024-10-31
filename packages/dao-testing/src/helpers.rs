use cosmwasm_std::{to_json_binary, Addr, Binary, Uint128};
use cw20::Cw20Coin;
use cw_multi_test::{App, Executor};
use cw_utils::Duration;
use dao_interface::state::{Admin, ModuleInstantiateInfo};
use dao_voting::threshold::ActiveThreshold;
use dao_voting_cw4::msg::GroupContract;

use crate::contracts::{
    cw20_base_contract, cw20_stake_contract, cw4_group_contract, dao_dao_core_contract,
    dao_voting_cw20_balance_contract, dao_voting_cw20_staked_contract, dao_voting_cw4_contract,
};

const CREATOR_ADDR: &str = "creator";

pub fn instantiate_with_cw20_balances_governance(
    app: &mut App,
    governance_code_id: u64,
    governance_instantiate: Binary,
    initial_balances: Option<Vec<Cw20Coin>>,
) -> Addr {
    let cw20_id = app.store_code(cw20_base_contract());
    let core_id = app.store_code(dao_dao_core_contract());
    let votemod_id = app.store_code(dao_voting_cw20_balance_contract());

    let initial_balances = initial_balances.unwrap_or_else(|| {
        vec![Cw20Coin {
            address: CREATOR_ADDR.to_string(),
            amount: Uint128::new(100_000_000),
        }]
    });

    // Collapse balances so that we can test double votes.
    let initial_balances: Vec<Cw20Coin> = {
        let mut already_seen = vec![];
        initial_balances
            .into_iter()
            .filter(|Cw20Coin { address, amount: _ }| {
                if already_seen.contains(address) {
                    false
                } else {
                    already_seen.push(address.clone());
                    true
                }
            })
            .collect()
    };

    let governance_instantiate = dao_interface::msg::InstantiateMsg {
        dao_uri: None,
        admin: None,
        name: "DAO DAO".to_string(),
        description: "A DAO that builds DAOs".to_string(),
        image_url: None,
        automatically_add_cw20s: true,
        automatically_add_cw721s: true,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: votemod_id,
            msg: to_json_binary(&dao_voting_cw20_balance::msg::InstantiateMsg {
                token_info: dao_voting_cw20_balance::msg::TokenInfo::New {
                    code_id: cw20_id,
                    label: "DAO DAO governance token".to_string(),
                    name: "DAO".to_string(),
                    symbol: "DAO".to_string(),
                    decimals: 6,
                    initial_balances,
                    marketing: None,
                },
            })
            .unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "DAO DAO voting module".to_string(),
        },
        proposal_modules_instantiate_info: vec![ModuleInstantiateInfo {
            code_id: governance_code_id,
            msg: governance_instantiate,
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "DAO DAO governance module".to_string(),
        }],
        initial_items: None,
    };

    app.instantiate_contract(
        core_id,
        Addr::unchecked(CREATOR_ADDR),
        &governance_instantiate,
        &[],
        "DAO DAO",
        None,
    )
    .unwrap()
}

pub fn instantiate_with_staked_balances_governance(
    app: &mut App,
    governance_code_id: u64,
    governance_instantiate: Binary,
    initial_balances: Option<Vec<Cw20Coin>>,
) -> Addr {
    let initial_balances = initial_balances.unwrap_or_else(|| {
        vec![Cw20Coin {
            address: CREATOR_ADDR.to_string(),
            amount: Uint128::new(100_000_000),
        }]
    });

    // Collapse balances so that we can test double votes.
    let initial_balances: Vec<Cw20Coin> = {
        let mut already_seen = vec![];
        initial_balances
            .into_iter()
            .filter(|Cw20Coin { address, amount: _ }| {
                if already_seen.contains(address) {
                    false
                } else {
                    already_seen.push(address.clone());
                    true
                }
            })
            .collect()
    };

    let cw20_id = app.store_code(cw20_base_contract());
    let cw20_stake_id = app.store_code(cw20_stake_contract());
    let staked_balances_voting_id = app.store_code(dao_voting_cw20_staked_contract());
    let core_contract_id = app.store_code(dao_dao_core_contract());

    let instantiate_core = dao_interface::msg::InstantiateMsg {
        dao_uri: None,
        admin: None,
        name: "DAO DAO".to_string(),
        description: "A DAO that builds DAOs".to_string(),
        image_url: None,
        automatically_add_cw20s: true,
        automatically_add_cw721s: false,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: staked_balances_voting_id,
            msg: to_json_binary(&dao_voting_cw20_staked::msg::InstantiateMsg {
                active_threshold: None,
                token_info: dao_voting_cw20_staked::msg::TokenInfo::New {
                    code_id: cw20_id,
                    label: "DAO DAO governance token.".to_string(),
                    name: "DAO DAO".to_string(),
                    symbol: "DAO".to_string(),
                    decimals: 6,
                    initial_balances: initial_balances.clone(),
                    marketing: None,
                    staking_code_id: cw20_stake_id,
                    unstaking_duration: Some(Duration::Height(6)),
                    initial_dao_balance: None,
                },
            })
            .unwrap(),
            admin: None,
            funds: vec![],
            label: "DAO DAO voting module".to_string(),
        },
        proposal_modules_instantiate_info: vec![ModuleInstantiateInfo {
            code_id: governance_code_id,
            label: "DAO DAO governance module.".to_string(),
            admin: Some(Admin::CoreModule {}),
            msg: governance_instantiate,
            funds: vec![],
        }],
        initial_items: None,
    };

    let core_addr = app
        .instantiate_contract(
            core_contract_id,
            Addr::unchecked(CREATOR_ADDR),
            &instantiate_core,
            &[],
            "DAO DAO",
            None,
        )
        .unwrap();

    let gov_state: dao_interface::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(
            core_addr.clone(),
            &dao_interface::msg::QueryMsg::DumpState {},
        )
        .unwrap();
    let voting_module = gov_state.voting_module;

    let staking_contract: Addr = app
        .wrap()
        .query_wasm_smart(
            voting_module.clone(),
            &dao_voting_cw20_staked::msg::QueryMsg::StakingContract {},
        )
        .unwrap();
    let token_contract: Addr = app
        .wrap()
        .query_wasm_smart(
            voting_module,
            &dao_interface::voting::Query::TokenContract {},
        )
        .unwrap();

    // Stake all the initial balances.
    for Cw20Coin { address, amount } in initial_balances {
        app.execute_contract(
            Addr::unchecked(address),
            token_contract.clone(),
            &cw20::Cw20ExecuteMsg::Send {
                contract: staking_contract.to_string(),
                amount,
                msg: to_json_binary(&cw20_stake::msg::ReceiveMsg::Stake {}).unwrap(),
            },
            &[],
        )
        .unwrap();
    }

    // Update the block so that those staked balances appear.
    app.update_block(|block| block.height += 1);

    core_addr
}

pub fn instantiate_with_staking_active_threshold(
    app: &mut App,
    code_id: u64,
    governance_instantiate: Binary,
    initial_balances: Option<Vec<Cw20Coin>>,
    active_threshold: Option<ActiveThreshold>,
) -> Addr {
    let cw20_id = app.store_code(cw20_base_contract());
    let cw20_staking_id = app.store_code(cw20_stake_contract());
    let governance_id = app.store_code(dao_dao_core_contract());
    let votemod_id = app.store_code(dao_voting_cw20_staked_contract());

    let initial_balances = initial_balances.unwrap_or_else(|| {
        vec![
            Cw20Coin {
                address: "blob".to_string(),
                amount: Uint128::new(100_000_000),
            },
            Cw20Coin {
                address: "blue".to_string(),
                amount: Uint128::new(100_000_000),
            },
        ]
    });

    let governance_instantiate = dao_interface::msg::InstantiateMsg {
        dao_uri: None,
        admin: None,
        name: "DAO DAO".to_string(),
        description: "A DAO that builds DAOs".to_string(),
        image_url: None,
        automatically_add_cw20s: true,
        automatically_add_cw721s: true,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: votemod_id,
            msg: to_json_binary(&dao_voting_cw20_staked::msg::InstantiateMsg {
                token_info: dao_voting_cw20_staked::msg::TokenInfo::New {
                    code_id: cw20_id,
                    label: "DAO DAO governance token".to_string(),
                    name: "DAO".to_string(),
                    symbol: "DAO".to_string(),
                    decimals: 6,
                    initial_balances,
                    marketing: None,
                    staking_code_id: cw20_staking_id,
                    unstaking_duration: None,
                    initial_dao_balance: None,
                },
                active_threshold,
            })
            .unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "DAO DAO voting module".to_string(),
        },
        proposal_modules_instantiate_info: vec![ModuleInstantiateInfo {
            code_id,
            msg: governance_instantiate,
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "DAO DAO governance module".to_string(),
        }],
        initial_items: None,
    };

    app.instantiate_contract(
        governance_id,
        Addr::unchecked(CREATOR_ADDR),
        &governance_instantiate,
        &[],
        "DAO DAO",
        None,
    )
    .unwrap()
}

pub fn instantiate_with_cw4_groups_governance(
    app: &mut App,
    core_code_id: u64,
    proposal_module_instantiate: Binary,
    initial_weights: Option<Vec<Cw20Coin>>,
) -> Addr {
    let cw4_id = app.store_code(cw4_group_contract());
    let core_id = app.store_code(dao_dao_core_contract());
    let votemod_id = app.store_code(dao_voting_cw4_contract());

    let initial_weights = initial_weights.unwrap_or_default();

    // Remove duplicates so that we can test duplicate voting.
    let initial_weights: Vec<cw4::Member> = {
        let mut already_seen = vec![];
        initial_weights
            .into_iter()
            .filter(|Cw20Coin { address, .. }| {
                if already_seen.contains(address) {
                    false
                } else {
                    already_seen.push(address.clone());
                    true
                }
            })
            .map(|Cw20Coin { address, amount }| cw4::Member {
                addr: address,
                weight: amount.u128() as u64,
            })
            .collect()
    };

    let governance_instantiate = dao_interface::msg::InstantiateMsg {
        dao_uri: None,
        admin: None,
        name: "DAO DAO".to_string(),
        description: "A DAO that builds DAOs".to_string(),
        image_url: None,
        automatically_add_cw20s: true,
        automatically_add_cw721s: true,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: votemod_id,
            msg: to_json_binary(&dao_voting_cw4::msg::InstantiateMsg {
                group_contract: GroupContract::New {
                    cw4_group_code_id: cw4_id,
                    initial_members: initial_weights,
                },
            })
            .unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "DAO DAO voting module".to_string(),
        },
        proposal_modules_instantiate_info: vec![ModuleInstantiateInfo {
            code_id: core_code_id,
            msg: proposal_module_instantiate,
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "DAO DAO governance module".to_string(),
        }],
        initial_items: None,
    };

    let addr = app
        .instantiate_contract(
            core_id,
            Addr::unchecked(CREATOR_ADDR),
            &governance_instantiate,
            &[],
            "DAO DAO",
            None,
        )
        .unwrap();

    // Update the block so that weights appear.
    app.update_block(|block| block.height += 1);

    addr
}
