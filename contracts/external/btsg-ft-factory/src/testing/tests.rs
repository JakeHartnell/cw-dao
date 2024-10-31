use cosmwasm_std::{
    coins,
    testing::{mock_dependencies, mock_env},
    to_json_binary, Addr, Uint128, WasmMsg,
};
use cw_multi_test::Executor;
use cw_utils::Duration;
use dao_interface::{
    state::{Admin, ModuleInstantiateInfo},
    token::InitialBalance,
};
use dao_testing::contracts::{dao_dao_core_contract, dao_proposal_single_contract};

use crate::{
    bitsong::{Coin, MsgMint, MsgSetUri},
    contract::{migrate, CONTRACT_NAME, CONTRACT_VERSION},
    msg::{ExecuteMsg, MigrateMsg, NewFanToken},
    testing::is_error,
};

use super::{setup_test, CommonTest, STAKER};

/// I can create a new fantoken on DAO creation.
#[test]
fn test_issue_fantoken() -> anyhow::Result<()> {
    let CommonTest {
        mut app,
        factory,
        module_id,
        ..
    } = setup_test();

    let core_id = app.store_code(dao_dao_core_contract());
    let proposal_single_id = app.store_code(dao_proposal_single_contract());

    let initial_balances = vec![InitialBalance {
        amount: Uint128::new(100),
        address: STAKER.to_string(),
    }];

    let governance_instantiate = dao_interface::msg::InstantiateMsg {
        dao_uri: None,
        admin: None,
        name: "DAO DAO".to_string(),
        description: "A DAO that builds DAOs".to_string(),
        image_url: None,
        automatically_add_cw20s: true,
        automatically_add_cw721s: true,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: module_id,
            msg: to_json_binary(&dao_voting_token_staked::msg::InstantiateMsg {
                token_info: dao_voting_token_staked::msg::TokenInfo::Factory(to_json_binary(
                    &WasmMsg::Execute {
                        contract_addr: factory.to_string(),
                        msg: to_json_binary(&ExecuteMsg::Issue(NewFanToken {
                            symbol: "FAN".to_string(),
                            name: "Fantoken".to_string(),
                            max_supply: Uint128::new(1_000_000_000_000_000_000),
                            uri: "".to_string(),
                            initial_balances,
                            initial_dao_balance: Some(Uint128::new(100_000_000)),
                        }))?,
                        funds: vec![],
                    },
                )?),
                unstaking_duration: None,
                active_threshold: None,
            })
            .unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "DAO DAO voting module".to_string(),
        },
        proposal_modules_instantiate_info: vec![ModuleInstantiateInfo {
            code_id: proposal_single_id,
            msg: to_json_binary(&dao_proposal_single::msg::InstantiateMsg {
                threshold: dao_voting::threshold::Threshold::AbsoluteCount {
                    threshold: Uint128::new(100),
                },
                max_voting_period: Duration::Time(86400),
                min_voting_period: None,
                only_members_execute: true,
                allow_revoting: false,
                pre_propose_info: dao_voting::pre_propose::PreProposeInfo::AnyoneMayPropose {},
                close_proposal_on_execution_failure: true,
                veto: None,
            })?,
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "DAO DAO governance module".to_string(),
        }],
        initial_items: None,
    };

    let dao = app
        .instantiate_contract(
            core_id,
            Addr::unchecked(STAKER),
            &governance_instantiate,
            &[],
            "DAO DAO",
            None,
        )
        .unwrap();

    let voting_module: Addr = app
        .wrap()
        .query_wasm_smart(dao, &dao_interface::msg::QueryMsg::VotingModule {})
        .unwrap();

    let denom_res: dao_interface::voting::DenomResponse = app
        .wrap()
        .query_wasm_smart(
            voting_module,
            &dao_voting_token_staked::msg::QueryMsg::Denom {},
        )
        .unwrap();

    // first fantoken created has the denom "fantoken1"
    assert_eq!(denom_res.denom, "fantoken1");

    Ok(())
}

/// I can create a new fantoken on DAO creation with initial balances.
#[test]
fn test_initial_fantoken_balances() -> anyhow::Result<()> {
    let CommonTest {
        mut app,
        factory,
        module_id,
        ..
    } = setup_test();

    let core_id = app.store_code(dao_dao_core_contract());
    let proposal_single_id = app.store_code(dao_proposal_single_contract());

    let initial_balances = vec![InitialBalance {
        amount: Uint128::new(100),
        address: STAKER.to_string(),
    }];

    let governance_instantiate = dao_interface::msg::InstantiateMsg {
        dao_uri: None,
        admin: None,
        name: "DAO DAO".to_string(),
        description: "A DAO that builds DAOs".to_string(),
        image_url: None,
        automatically_add_cw20s: true,
        automatically_add_cw721s: true,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: module_id,
            msg: to_json_binary(&dao_voting_token_staked::msg::InstantiateMsg {
                token_info: dao_voting_token_staked::msg::TokenInfo::Factory(to_json_binary(
                    &WasmMsg::Execute {
                        contract_addr: factory.to_string(),
                        msg: to_json_binary(&ExecuteMsg::Issue(NewFanToken {
                            symbol: "FAN".to_string(),
                            name: "Fantoken".to_string(),
                            max_supply: Uint128::new(1_000_000_000_000_000_000),
                            uri: "".to_string(),
                            initial_balances,
                            initial_dao_balance: Some(Uint128::new(100_000_000)),
                        }))?,
                        funds: vec![],
                    },
                )?),
                unstaking_duration: None,
                active_threshold: None,
            })
            .unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "DAO DAO voting module".to_string(),
        },
        proposal_modules_instantiate_info: vec![ModuleInstantiateInfo {
            code_id: proposal_single_id,
            msg: to_json_binary(&dao_proposal_single::msg::InstantiateMsg {
                threshold: dao_voting::threshold::Threshold::AbsoluteCount {
                    threshold: Uint128::new(100),
                },
                max_voting_period: Duration::Time(86400),
                min_voting_period: None,
                only_members_execute: true,
                allow_revoting: false,
                pre_propose_info: dao_voting::pre_propose::PreProposeInfo::AnyoneMayPropose {},
                close_proposal_on_execution_failure: true,
                veto: None,
            })?,
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "DAO DAO governance module".to_string(),
        }],
        initial_items: None,
    };

    let dao = app
        .instantiate_contract(
            core_id,
            Addr::unchecked(STAKER),
            &governance_instantiate,
            &[],
            "DAO DAO",
            None,
        )
        .unwrap();

    let voting_module: Addr = app
        .wrap()
        .query_wasm_smart(&dao, &dao_interface::msg::QueryMsg::VotingModule {})
        .unwrap();

    let denom_res: dao_interface::voting::DenomResponse = app
        .wrap()
        .query_wasm_smart(
            voting_module,
            &dao_voting_token_staked::msg::QueryMsg::Denom {},
        )
        .unwrap();

    // verify DAO has initial balance
    let dao_balance = app.wrap().query_balance(&dao, &denom_res.denom).unwrap();
    assert_eq!(dao_balance.amount, Uint128::new(100_000_000));

    // verify staker has initial balance
    let staker_balance = app.wrap().query_balance(STAKER, &denom_res.denom).unwrap();
    assert_eq!(staker_balance.amount, Uint128::new(100));

    Ok(())
}

/// The minter and authority are set to the DAO.
#[test]
fn test_fantoken_minter_and_authority_set_to_dao() -> anyhow::Result<()> {
    let CommonTest {
        mut app,
        factory,
        module_id,
        ..
    } = setup_test();

    let core_id = app.store_code(dao_dao_core_contract());
    let proposal_single_id = app.store_code(dao_proposal_single_contract());

    let initial_balances = vec![InitialBalance {
        amount: Uint128::new(100),
        address: STAKER.to_string(),
    }];

    let governance_instantiate = dao_interface::msg::InstantiateMsg {
        dao_uri: None,
        admin: None,
        name: "DAO DAO".to_string(),
        description: "A DAO that builds DAOs".to_string(),
        image_url: None,
        automatically_add_cw20s: true,
        automatically_add_cw721s: true,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: module_id,
            msg: to_json_binary(&dao_voting_token_staked::msg::InstantiateMsg {
                token_info: dao_voting_token_staked::msg::TokenInfo::Factory(to_json_binary(
                    &WasmMsg::Execute {
                        contract_addr: factory.to_string(),
                        msg: to_json_binary(&ExecuteMsg::Issue(NewFanToken {
                            symbol: "FAN".to_string(),
                            name: "Fantoken".to_string(),
                            max_supply: Uint128::new(1_000_000_000_000_000_000),
                            uri: "".to_string(),
                            initial_balances,
                            initial_dao_balance: Some(Uint128::new(100_000_000)),
                        }))?,
                        funds: vec![],
                    },
                )?),
                unstaking_duration: None,
                active_threshold: None,
            })
            .unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "DAO DAO voting module".to_string(),
        },
        proposal_modules_instantiate_info: vec![ModuleInstantiateInfo {
            code_id: proposal_single_id,
            msg: to_json_binary(&dao_proposal_single::msg::InstantiateMsg {
                threshold: dao_voting::threshold::Threshold::AbsoluteCount {
                    threshold: Uint128::new(100),
                },
                max_voting_period: Duration::Time(86400),
                min_voting_period: None,
                only_members_execute: true,
                allow_revoting: false,
                pre_propose_info: dao_voting::pre_propose::PreProposeInfo::AnyoneMayPropose {},
                close_proposal_on_execution_failure: true,
                veto: None,
            })?,
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "DAO DAO governance module".to_string(),
        }],
        initial_items: None,
    };

    let dao = app
        .instantiate_contract(
            core_id,
            Addr::unchecked(STAKER),
            &governance_instantiate,
            &[],
            "DAO DAO",
            None,
        )
        .unwrap();

    let voting_module: Addr = app
        .wrap()
        .query_wasm_smart(&dao, &dao_interface::msg::QueryMsg::VotingModule {})
        .unwrap();

    let denom_res: dao_interface::voting::DenomResponse = app
        .wrap()
        .query_wasm_smart(
            voting_module,
            &dao_voting_token_staked::msg::QueryMsg::Denom {},
        )
        .unwrap();

    // attempt to mint with factory that created the token, and fail
    let res = app.execute(
        factory.clone(),
        MsgMint {
            recipient: STAKER.to_string(),
            coin: Some(Coin {
                amount: "100".to_string(),
                denom: denom_res.denom.clone(),
            }),
            minter: factory.to_string(),
        }
        .into(),
    );
    is_error!(res => "Minter unauthorized");

    // verify minter is the DAO
    app.execute(
        dao.clone(),
        MsgMint {
            recipient: STAKER.to_string(),
            coin: Some(Coin {
                amount: "100".to_string(),
                denom: denom_res.denom.clone(),
            }),
            minter: dao.to_string(),
        }
        .into(),
    )
    .unwrap();

    // attempt to change URI with factory that created the token, and fail
    let res = app.execute(
        factory.clone(),
        MsgSetUri {
            authority: factory.to_string(),
            denom: denom_res.denom.clone(),
            uri: "https://example.com".to_string(),
        }
        .into(),
    );
    is_error!(res => "Authority unauthorized");

    // verify authority is the DAO
    app.execute(
        dao.clone(),
        MsgSetUri {
            authority: dao.to_string(),
            denom: denom_res.denom.clone(),
            uri: "https://example.com".to_string(),
        }
        .into(),
    )
    .unwrap();

    // verify staker has new balance
    let staker_balance = app.wrap().query_balance(STAKER, &denom_res.denom).unwrap();
    assert_eq!(staker_balance.amount, Uint128::new(200));

    Ok(())
}

/// A staker can stake fantokens.
#[test]
fn test_fantoken_can_be_staked() -> anyhow::Result<()> {
    let CommonTest {
        mut app,
        factory,
        module_id,
        ..
    } = setup_test();

    let core_id = app.store_code(dao_dao_core_contract());
    let proposal_single_id = app.store_code(dao_proposal_single_contract());

    let initial_balances = vec![InitialBalance {
        amount: Uint128::new(100),
        address: STAKER.to_string(),
    }];

    let governance_instantiate = dao_interface::msg::InstantiateMsg {
        dao_uri: None,
        admin: None,
        name: "DAO DAO".to_string(),
        description: "A DAO that builds DAOs".to_string(),
        image_url: None,
        automatically_add_cw20s: true,
        automatically_add_cw721s: true,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: module_id,
            msg: to_json_binary(&dao_voting_token_staked::msg::InstantiateMsg {
                token_info: dao_voting_token_staked::msg::TokenInfo::Factory(to_json_binary(
                    &WasmMsg::Execute {
                        contract_addr: factory.to_string(),
                        msg: to_json_binary(&ExecuteMsg::Issue(NewFanToken {
                            symbol: "FAN".to_string(),
                            name: "Fantoken".to_string(),
                            max_supply: Uint128::new(1_000_000_000_000_000_000),
                            uri: "".to_string(),
                            initial_balances,
                            initial_dao_balance: Some(Uint128::new(100_000_000)),
                        }))?,
                        funds: vec![],
                    },
                )?),
                unstaking_duration: None,
                active_threshold: None,
            })
            .unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "DAO DAO voting module".to_string(),
        },
        proposal_modules_instantiate_info: vec![ModuleInstantiateInfo {
            code_id: proposal_single_id,
            msg: to_json_binary(&dao_proposal_single::msg::InstantiateMsg {
                threshold: dao_voting::threshold::Threshold::AbsoluteCount {
                    threshold: Uint128::new(100),
                },
                max_voting_period: Duration::Time(86400),
                min_voting_period: None,
                only_members_execute: true,
                allow_revoting: false,
                pre_propose_info: dao_voting::pre_propose::PreProposeInfo::AnyoneMayPropose {},
                close_proposal_on_execution_failure: true,
                veto: None,
            })?,
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "DAO DAO governance module".to_string(),
        }],
        initial_items: None,
    };

    let dao = app
        .instantiate_contract(
            core_id,
            Addr::unchecked(STAKER),
            &governance_instantiate,
            &[],
            "DAO DAO",
            None,
        )
        .unwrap();

    let voting_module: Addr = app
        .wrap()
        .query_wasm_smart(dao, &dao_interface::msg::QueryMsg::VotingModule {})
        .unwrap();

    let denom_res: dao_interface::voting::DenomResponse = app
        .wrap()
        .query_wasm_smart(
            &voting_module,
            &dao_voting_token_staked::msg::QueryMsg::Denom {},
        )
        .unwrap();

    // verify staker voting power is 0
    let vp: dao_interface::voting::VotingPowerAtHeightResponse = app.wrap().query_wasm_smart(
        &voting_module,
        &dao_interface::voting::Query::VotingPowerAtHeight {
            address: STAKER.to_string(),
            height: None,
        },
    )?;
    assert_eq!(vp.power, Uint128::new(0));

    // stake from staker
    app.execute_contract(
        Addr::unchecked(STAKER),
        voting_module.clone(),
        &dao_voting_token_staked::msg::ExecuteMsg::Stake {},
        &coins(100, denom_res.denom),
    )?;

    // next block so voting power is updated
    app.update_block(|b| b.height += 1);

    // verify staker voting power is 100
    let vp: dao_interface::voting::VotingPowerAtHeightResponse = app.wrap().query_wasm_smart(
        &voting_module,
        &dao_interface::voting::Query::VotingPowerAtHeight {
            address: STAKER.to_string(),
            height: None,
        },
    )?;
    assert_eq!(vp.power, Uint128::new(100));

    Ok(())
}

#[test]
pub fn test_migrate_update_version() {
    let mut deps = mock_dependencies();
    cw2::set_contract_version(&mut deps.storage, "my-contract", "1.0.0").unwrap();

    migrate(deps.as_mut(), mock_env(), MigrateMsg {}).unwrap();
    let version = cw2::get_contract_version(&deps.storage).unwrap();
    assert_eq!(version.version, CONTRACT_VERSION);
    assert_eq!(version.contract, CONTRACT_NAME);

    // migrate again, should do nothing
    migrate(deps.as_mut(), mock_env(), MigrateMsg {}).unwrap();
    let version = cw2::get_contract_version(&deps.storage).unwrap();
    assert_eq!(version.version, CONTRACT_VERSION);
    assert_eq!(version.contract, CONTRACT_NAME);
}
