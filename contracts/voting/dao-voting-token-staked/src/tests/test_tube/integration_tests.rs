use crate::{
    msg::{ExecuteMsg, InstantiateMsg, QueryMsg, TokenInfo},
    tests::test_tube::test_env::TokenVotingContract,
    ContractError,
};
use cosmwasm_std::{to_json_binary, Addr, Coin, Decimal, Uint128, WasmMsg};
use cw_ownable::Ownership;
use cw_tokenfactory_issuer::msg::{DenomUnit, QueryMsg as IssuerQueryMsg};
use cw_utils::Duration;
use dao_interface::{
    msg::QueryMsg as DaoQueryMsg,
    state::{Admin, ModuleInstantiateInfo},
    token::{InitialBalance, NewDenomMetadata, NewTokenInfo},
};
use dao_testing::test_tube::{cw_tokenfactory_issuer::TokenfactoryIssuer, dao_dao_core::DaoCore};
use dao_voting::{
    pre_propose::PreProposeInfo,
    threshold::{ActiveThreshold, ActiveThresholdError, PercentageThreshold, Threshold},
};
use osmosis_test_tube::{
    osmosis_std::types::cosmos::bank::v1beta1::QueryBalanceRequest, Account, OsmosisTestApp,
    RunnerError,
};

use super::test_env::{TestEnv, TestEnvBuilder, DENOM};

#[test]
fn test_full_integration_correct_setup() {
    let app = OsmosisTestApp::new();
    let env = TestEnvBuilder::new();
    let TestEnv { dao, tf_issuer, .. } = env.full_dao_setup(&app);

    // Issuer owner should be set to the DAO
    let issuer_admin = tf_issuer
        .query::<cw_ownable::Ownership<Addr>>(&cw_tokenfactory_issuer::msg::QueryMsg::Ownership {})
        .unwrap()
        .owner;
    assert_eq!(
        issuer_admin,
        Some(Addr::unchecked(dao.unwrap().contract_addr))
    );
}

#[test]
fn test_stake_unstake_new_denom() {
    let app = OsmosisTestApp::new();
    let env = TestEnvBuilder::new();
    let TestEnv {
        vp_contract,
        accounts,
        ..
    } = env.full_dao_setup(&app);

    let denom = vp_contract.query_denom().unwrap().denom;

    // Stake 100 tokens
    let stake_msg = ExecuteMsg::Stake {};
    vp_contract
        .execute(&stake_msg, &[Coin::new(100, denom)], &accounts[0])
        .unwrap();

    app.increase_time(1);

    // Query voting power
    let voting_power = vp_contract.query_vp(&accounts[0].address(), None).unwrap();
    assert_eq!(voting_power.power, Uint128::new(100));

    // DAO is active (default threshold is absolute count of 75)
    let active = vp_contract.query_active().unwrap().active;
    assert!(active);

    // Unstake 50 tokens
    let unstake_msg = ExecuteMsg::Unstake {
        amount: Uint128::new(50),
    };
    vp_contract
        .execute(&unstake_msg, &[], &accounts[0])
        .unwrap();
    app.increase_time(1);
    let voting_power = vp_contract.query_vp(&accounts[0].address(), None).unwrap();
    assert_eq!(voting_power.power, Uint128::new(50));

    // DAO is not active
    let active = vp_contract.query_active().unwrap().active;
    assert!(!active);

    // Can't claim before unstaking period (2 seconds)
    vp_contract
        .execute(&ExecuteMsg::Claim {}, &[], &accounts[0])
        .unwrap_err();

    // Pass time, unstaking duration is set to 2 seconds
    app.increase_time(5);
    vp_contract
        .execute(&ExecuteMsg::Claim {}, &[], &accounts[0])
        .unwrap();
}

#[test]
fn test_instantiate_no_dao_balance() {
    let app = OsmosisTestApp::new();
    let env = TestEnvBuilder::new().default_setup(&app);
    let tf_issuer_id = env.get_tf_issuer_code_id();

    let dao = app
        .init_account(&[Coin::new(100000000000, "uosmo")])
        .unwrap();
    let dao_addr = &dao.address();

    let vp_contract = env
        .instantiate(
            &InstantiateMsg {
                token_info: TokenInfo::New(NewTokenInfo {
                    token_issuer_code_id: tf_issuer_id,
                    subdenom: "ucat".to_string(),
                    metadata: Some(NewDenomMetadata {
                        description: "Awesome token, get it meow!".to_string(),
                        additional_denom_units: Some(vec![DenomUnit {
                            denom: "cat".to_string(),
                            exponent: 6,
                            aliases: vec![],
                        }]),
                        display: "cat".to_string(),
                        name: "Cat Token".to_string(),
                        symbol: "CAT".to_string(),
                    }),
                    initial_balances: vec![InitialBalance {
                        amount: Uint128::new(100),
                        address: env.accounts[0].address(),
                    }],
                    initial_dao_balance: None,
                }),
                unstaking_duration: None,
                active_threshold: None,
            },
            dao,
        )
        .unwrap();

    let denom = vp_contract.query_denom().unwrap().denom;

    // Check balances
    // Account 0
    let bal = env
        .bank()
        .query_balance(&QueryBalanceRequest {
            address: env.accounts[0].address(),
            denom: denom.clone(),
        })
        .unwrap();
    assert_eq!(bal.balance.unwrap().amount, Uint128::new(100).to_string());

    // DAO
    let bal = env
        .bank()
        .query_balance(&QueryBalanceRequest {
            address: dao_addr.to_string(),
            denom,
        })
        .unwrap();
    assert_eq!(bal.balance.unwrap().amount, Uint128::zero().to_string());
}

#[test]
fn test_instantiate_no_metadata() {
    let app = OsmosisTestApp::new();
    let env = TestEnvBuilder::new().default_setup(&app);
    let tf_issuer_id = env.get_tf_issuer_code_id();

    let dao = app
        .init_account(&[Coin::new(100000000000, "uosmo")])
        .unwrap();

    env.instantiate(
        &InstantiateMsg {
            token_info: TokenInfo::New(NewTokenInfo {
                token_issuer_code_id: tf_issuer_id,
                subdenom: "ucat".to_string(),
                metadata: None,
                initial_balances: vec![InitialBalance {
                    amount: Uint128::new(100),
                    address: env.accounts[0].address(),
                }],
                initial_dao_balance: None,
            }),
            unstaking_duration: None,
            active_threshold: None,
        },
        dao,
    )
    .unwrap();
}

#[test]
fn test_instantiate_invalid_metadata_fails() {
    let app = OsmosisTestApp::new();
    let env = TestEnvBuilder::new().default_setup(&app);
    let tf_issuer_id = env.get_tf_issuer_code_id();

    let dao = app
        .init_account(&[Coin::new(100000000000, "uosmo")])
        .unwrap();

    env.instantiate(
        &InstantiateMsg {
            token_info: TokenInfo::New(NewTokenInfo {
                token_issuer_code_id: tf_issuer_id,
                subdenom: "cat".to_string(),
                metadata: Some(NewDenomMetadata {
                    description: "Awesome token, get it meow!".to_string(),
                    additional_denom_units: Some(vec![DenomUnit {
                        denom: "cat".to_string(),
                        // Exponent 0 is automatically set
                        exponent: 0,
                        aliases: vec![],
                    }]),
                    display: "cat".to_string(),
                    name: "Cat Token".to_string(),
                    symbol: "CAT".to_string(),
                }),
                initial_balances: vec![InitialBalance {
                    amount: Uint128::new(100),
                    address: env.accounts[0].address(),
                }],
                initial_dao_balance: None,
            }),
            unstaking_duration: None,
            active_threshold: None,
        },
        dao,
    )
    .unwrap_err();
}

#[test]
fn test_instantiate_invalid_active_threshold_count_fails() {
    let app = OsmosisTestApp::new();
    let env = TestEnvBuilder::new().default_setup(&app);
    let tf_issuer_id = env.get_tf_issuer_code_id();

    let dao = app
        .init_account(&[Coin::new(100000000000, "uosmo")])
        .unwrap();

    let err = env
        .instantiate(
            &InstantiateMsg {
                token_info: TokenInfo::New(NewTokenInfo {
                    token_issuer_code_id: tf_issuer_id,
                    subdenom: "cat".to_string(),
                    metadata: Some(NewDenomMetadata {
                        description: "Awesome token, get it meow!".to_string(),
                        additional_denom_units: Some(vec![DenomUnit {
                            denom: "cat".to_string(),
                            // Exponent 0 is automatically set
                            exponent: 0,
                            aliases: vec![],
                        }]),
                        display: "cat".to_string(),
                        name: "Cat Token".to_string(),
                        symbol: "CAT".to_string(),
                    }),
                    initial_balances: vec![InitialBalance {
                        amount: Uint128::new(100),
                        address: env.accounts[0].address(),
                    }],
                    initial_dao_balance: None,
                }),
                unstaking_duration: None,
                active_threshold: Some(ActiveThreshold::AbsoluteCount {
                    count: Uint128::new(1000),
                }),
            },
            dao,
        )
        .unwrap_err();

    assert_eq!(
        err,
        TokenVotingContract::execute_submessage_error(ContractError::ActiveThresholdError(
            ActiveThresholdError::InvalidAbsoluteCount {}
        ))
    );
}

#[test]
fn test_instantiate_no_initial_balances_fails() {
    let app = OsmosisTestApp::new();
    let env = TestEnvBuilder::new().default_setup(&app);
    let tf_issuer_id = env.get_tf_issuer_code_id();
    let dao = app
        .init_account(&[Coin::new(10000000000000, "uosmo")])
        .unwrap();

    let err = env
        .instantiate(
            &InstantiateMsg {
                token_info: TokenInfo::New(NewTokenInfo {
                    token_issuer_code_id: tf_issuer_id,
                    subdenom: "ucat".to_string(),
                    metadata: Some(NewDenomMetadata {
                        description: "Awesome token, get it meow!".to_string(),
                        additional_denom_units: Some(vec![DenomUnit {
                            denom: "cat".to_string(),
                            exponent: 6,
                            aliases: vec![],
                        }]),
                        display: "cat".to_string(),
                        name: "Cat Token".to_string(),
                        symbol: "CAT".to_string(),
                    }),
                    initial_balances: vec![],
                    initial_dao_balance: Some(Uint128::new(100000)),
                }),
                unstaking_duration: None,
                active_threshold: None,
            },
            dao,
        )
        .unwrap_err();
    assert_eq!(
        err,
        TokenVotingContract::execute_submessage_error(ContractError::InitialBalancesError {})
    );
}

#[test]
fn test_factory() {
    let app = OsmosisTestApp::new();
    let env = TestEnvBuilder::new();
    let TestEnv {
        tf_issuer,
        vp_contract,
        proposal_single,
        custom_factory,
        accounts,
        ..
    } = env.full_dao_setup(&app);

    let factory_addr = custom_factory.unwrap().contract_addr.to_string();

    // Instantiate a new voting contract using the factory pattern
    let msg = dao_interface::msg::InstantiateMsg {
        dao_uri: None,
        admin: None,
        name: "DAO DAO".to_string(),
        description: "A DAO that makes DAO tooling".to_string(),
        image_url: None,
        automatically_add_cw20s: false,
        automatically_add_cw721s: false,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: vp_contract.code_id,
            msg: to_json_binary(&InstantiateMsg {
                token_info: TokenInfo::Factory(
                    to_json_binary(&WasmMsg::Execute {
                        contract_addr: factory_addr.clone(),
                        msg: to_json_binary(
                            &dao_test_custom_factory::msg::ExecuteMsg::TokenFactoryFactory(
                                NewTokenInfo {
                                    token_issuer_code_id: tf_issuer.code_id,
                                    subdenom: DENOM.to_string(),
                                    metadata: None,
                                    initial_balances: vec![InitialBalance {
                                        address: accounts[0].address(),
                                        amount: Uint128::new(100),
                                    }],
                                    initial_dao_balance: None,
                                },
                            ),
                        )
                        .unwrap(),
                        funds: vec![],
                    })
                    .unwrap(),
                ),
                unstaking_duration: Some(Duration::Time(2)),
                active_threshold: Some(ActiveThreshold::AbsoluteCount {
                    count: Uint128::new(75),
                }),
            })
            .unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "DAO DAO Voting Module".to_string(),
        },
        proposal_modules_instantiate_info: vec![ModuleInstantiateInfo {
            code_id: proposal_single.unwrap().code_id,
            msg: to_json_binary(&dao_proposal_single::msg::InstantiateMsg {
                min_voting_period: None,
                threshold: Threshold::ThresholdQuorum {
                    threshold: PercentageThreshold::Majority {},
                    quorum: PercentageThreshold::Percent(Decimal::percent(35)),
                },
                max_voting_period: Duration::Time(432000),
                allow_revoting: false,
                only_members_execute: true,
                close_proposal_on_execution_failure: false,
                pre_propose_info: PreProposeInfo::AnyoneMayPropose {},
                veto: None,
                delegation_module: None,
            })
            .unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "DAO DAO Proposal Module".to_string(),
        }],
        initial_items: None,
    };

    // Instantiate DAO succeeds
    let dao = DaoCore::new(&app, &msg, &accounts[0], &vec![]).unwrap();

    // Query voting module
    let voting_module: Addr = dao.query(&DaoQueryMsg::VotingModule {}).unwrap();
    let voting =
        TokenVotingContract::new_with_values(&app, vp_contract.code_id, voting_module.to_string())
            .unwrap();

    // Query denom
    let denom = voting.query_denom().unwrap().denom;

    // Query token contract
    let token_contract: Addr = voting.query(&QueryMsg::TokenContract {}).unwrap();

    // Check the TF denom is as expected
    assert_eq!(denom, format!("factory/{}/{}", token_contract, DENOM));

    // Check issuer ownership is the DAO and the ModuleInstantiateCallback
    // has successfully accepted ownership.
    let issuer =
        TokenfactoryIssuer::new_with_values(&app, tf_issuer.code_id, token_contract.to_string())
            .unwrap();
    let ownership: Ownership<Addr> = issuer.query(&IssuerQueryMsg::Ownership {}).unwrap();
    let owner = ownership.owner.unwrap();
    assert_eq!(owner, dao.contract_addr);
}

#[test]
fn test_factory_funds_pass_through() {
    let app = OsmosisTestApp::new();
    let env = TestEnvBuilder::new();
    let TestEnv {
        tf_issuer,
        vp_contract,
        proposal_single,
        custom_factory,
        accounts,
        ..
    } = env.full_dao_setup(&app);

    let factory_addr = custom_factory.unwrap().contract_addr.to_string();

    // Instantiate a new voting contract using the factory pattern
    let mut msg = dao_interface::msg::InstantiateMsg {
        dao_uri: None,
        admin: None,
        name: "DAO DAO".to_string(),
        description: "A DAO that makes DAO tooling".to_string(),
        image_url: None,
        automatically_add_cw20s: false,
        automatically_add_cw721s: false,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: vp_contract.code_id,
            msg: to_json_binary(&InstantiateMsg {
                token_info: TokenInfo::Factory(
                    to_json_binary(&WasmMsg::Execute {
                        contract_addr: factory_addr.clone(),
                        msg: to_json_binary(
                            &dao_test_custom_factory::msg::ExecuteMsg::TokenFactoryFactoryWithFunds(
                                NewTokenInfo {
                                    token_issuer_code_id: tf_issuer.code_id,
                                    subdenom: DENOM.to_string(),
                                    metadata: None,
                                    initial_balances: vec![InitialBalance {
                                        address: accounts[0].address(),
                                        amount: Uint128::new(100),
                                    }],
                                    initial_dao_balance: None,
                                },
                            ),
                        )
                        .unwrap(),
                        funds: vec![],
                    })
                    .unwrap(),
                ),
                unstaking_duration: Some(Duration::Time(2)),
                active_threshold: Some(ActiveThreshold::AbsoluteCount {
                    count: Uint128::new(75),
                }),
            })
            .unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "DAO DAO Voting Module".to_string(),
        },
        proposal_modules_instantiate_info: vec![ModuleInstantiateInfo {
            code_id: proposal_single.unwrap().code_id,
            msg: to_json_binary(&dao_proposal_single::msg::InstantiateMsg {
                min_voting_period: None,
                threshold: Threshold::ThresholdQuorum {
                    threshold: PercentageThreshold::Majority {},
                    quorum: PercentageThreshold::Percent(Decimal::percent(35)),
                },
                max_voting_period: Duration::Time(432000),
                allow_revoting: false,
                only_members_execute: true,
                close_proposal_on_execution_failure: false,
                pre_propose_info: PreProposeInfo::AnyoneMayPropose {},
                veto: None,
                delegation_module: None,
            })
            .unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "DAO DAO Proposal Module".to_string(),
        }],
        initial_items: None,
    };

    // Instantiate DAO fails because no funds to create the token were sent
    let err = DaoCore::new(&app, &msg, &accounts[0], &vec![]).unwrap_err();

    // Check error is no funds sent
    assert_eq!(
        err,
        RunnerError::ExecuteError {
            msg: "failed to execute message; message index: 0: dispatch: submessages: dispatch: submessages: No funds sent: execute wasm contract failed".to_string(),
        }
    );

    // Include funds in ModuleInstantiateInfo
    let funds = vec![Coin {
        denom: "uosmo".to_string(),
        amount: Uint128::new(100),
    }];
    msg.voting_module_instantiate_info = ModuleInstantiateInfo {
        code_id: vp_contract.code_id,
        msg: to_json_binary(&InstantiateMsg {
            token_info: TokenInfo::Factory(
                to_json_binary(&WasmMsg::Execute {
                    contract_addr: factory_addr,
                    msg: to_json_binary(
                        &dao_test_custom_factory::msg::ExecuteMsg::TokenFactoryFactoryWithFunds(
                            NewTokenInfo {
                                token_issuer_code_id: tf_issuer.code_id,
                                subdenom: DENOM.to_string(),
                                metadata: None,
                                initial_balances: vec![InitialBalance {
                                    address: accounts[0].address(),
                                    amount: Uint128::new(100),
                                }],
                                initial_dao_balance: None,
                            },
                        ),
                    )
                    .unwrap(),
                    funds: funds.clone(),
                })
                .unwrap(),
            ),
            unstaking_duration: Some(Duration::Time(2)),
            active_threshold: Some(ActiveThreshold::AbsoluteCount {
                count: Uint128::new(75),
            }),
        })
        .unwrap(),
        admin: Some(Admin::CoreModule {}),
        funds: funds.clone(),
        label: "DAO DAO Voting Module".to_string(),
    };

    // Creating the DAO now succeeds
    DaoCore::new(&app, &msg, &accounts[0], &funds).unwrap();
}

#[test]
fn test_factory_no_callback() {
    let app = OsmosisTestApp::new();
    let env = TestEnvBuilder::new();
    let TestEnv {
        vp_contract,
        proposal_single,
        custom_factory,
        accounts,
        ..
    } = env.full_dao_setup(&app);

    let factory_addr = custom_factory.unwrap().contract_addr.to_string();

    // Instantiate a new voting contract using the factory pattern
    let msg = dao_interface::msg::InstantiateMsg {
        dao_uri: None,
        admin: None,
        name: "DAO DAO".to_string(),
        description: "A DAO that makes DAO tooling".to_string(),
        image_url: None,
        automatically_add_cw20s: false,
        automatically_add_cw721s: false,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: vp_contract.code_id,
            msg: to_json_binary(&InstantiateMsg {
                token_info: TokenInfo::Factory(
                    to_json_binary(&WasmMsg::Execute {
                        contract_addr: factory_addr.clone(),
                        msg: to_json_binary(
                            &dao_test_custom_factory::msg::ExecuteMsg::TokenFactoryFactoryNoCallback{},
                        )
                        .unwrap(),
                        funds: vec![],
                    })
                    .unwrap(),
                ),
                unstaking_duration: Some(Duration::Time(2)),
                active_threshold: Some(ActiveThreshold::AbsoluteCount {
                    count: Uint128::new(75),
                }),
            })
            .unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "DAO DAO Voting Module".to_string(),
        },
        proposal_modules_instantiate_info: vec![ModuleInstantiateInfo {
            code_id: proposal_single.unwrap().code_id,
            msg: to_json_binary(&dao_proposal_single::msg::InstantiateMsg {
                min_voting_period: None,
                threshold: Threshold::ThresholdQuorum {
                    threshold: PercentageThreshold::Majority {},
                    quorum: PercentageThreshold::Percent(Decimal::percent(35)),
                },
                max_voting_period: Duration::Time(432000),
                allow_revoting: false,
                only_members_execute: true,
                close_proposal_on_execution_failure: false,
                pre_propose_info: PreProposeInfo::AnyoneMayPropose {},
                veto: None,
                delegation_module: None,
            })
            .unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "DAO DAO Proposal Module".to_string(),
        }],
        initial_items: None,
    };

    // Instantiate DAO fails because no callback
    let err = DaoCore::new(&app, &msg, &accounts[0], &vec![]).unwrap_err();

    // Check error is no reply data
    assert_eq!(
        err,
        RunnerError::ExecuteError {
            msg: "failed to execute message; message index: 0: dispatch: submessages: dispatch: submessages: reply: Invalid reply from sub-message: Missing reply data: execute wasm contract failed".to_string(),
        }
    );
}

#[test]
fn test_factory_wrong_callback() {
    let app = OsmosisTestApp::new();
    let env = TestEnvBuilder::new();
    let TestEnv {
        vp_contract,
        proposal_single,
        custom_factory,
        accounts,
        ..
    } = env.full_dao_setup(&app);

    let factory_addr = custom_factory.unwrap().contract_addr.to_string();
    // Instantiate a new voting contract using the factory pattern
    let msg = dao_interface::msg::InstantiateMsg {
        dao_uri: None,
        admin: None,
        name: "DAO DAO".to_string(),
        description: "A DAO that makes DAO tooling".to_string(),
        image_url: None,
        automatically_add_cw20s: false,
        automatically_add_cw721s: false,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: vp_contract.code_id,
            msg: to_json_binary(&InstantiateMsg {
                token_info: TokenInfo::Factory(
                    to_json_binary(&WasmMsg::Execute {
                        contract_addr: factory_addr.clone(),
                        msg: to_json_binary(
                            &dao_test_custom_factory::msg::ExecuteMsg::TokenFactoryFactoryWrongCallback{},
                        )
                        .unwrap(),
                        funds: vec![],
                    })
                    .unwrap(),
                ),
                unstaking_duration: Some(Duration::Time(2)),
                active_threshold: Some(ActiveThreshold::AbsoluteCount {
                    count: Uint128::new(75),
                }),
            })
            .unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "DAO DAO Voting Module".to_string(),
        },
        proposal_modules_instantiate_info: vec![ModuleInstantiateInfo {
            code_id: proposal_single.unwrap().code_id,
            msg: to_json_binary(&dao_proposal_single::msg::InstantiateMsg {
                min_voting_period: None,
                threshold: Threshold::ThresholdQuorum {
                    threshold: PercentageThreshold::Majority {},
                    quorum: PercentageThreshold::Percent(Decimal::percent(35)),
                },
                max_voting_period: Duration::Time(432000),
                allow_revoting: false,
                only_members_execute: true,
                close_proposal_on_execution_failure: false,
                pre_propose_info: PreProposeInfo::AnyoneMayPropose {},
                veto: None,
                delegation_module: None,
            })
            .unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "DAO DAO Proposal Module".to_string(),
        }],
        initial_items: None,
    };

    // Instantiate DAO fails because of wrong callback
    let err = DaoCore::new(&app, &msg, &accounts[0], &vec![]).unwrap_err();

    // Check error is wrong reply type
    assert_eq!(
        err,
        RunnerError::ExecuteError {
            msg: "failed to execute message; message index: 0: dispatch: submessages: dispatch: submessages: reply: Error parsing into type dao_interface::token::TokenFactoryCallback: unknown field `nft_contract`, expected one of `denom`, `token_contract`, `module_instantiate_callback`: execute wasm contract failed".to_string(),
        }
    );
}
