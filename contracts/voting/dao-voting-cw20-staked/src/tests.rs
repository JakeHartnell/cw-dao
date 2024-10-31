use cosmwasm_std::{
    testing::{mock_dependencies, mock_env},
    to_json_binary, Addr, CosmosMsg, Decimal, Uint128, WasmMsg,
};
use cw2::ContractVersion;
use cw20::{BalanceResponse, Cw20Coin, MinterResponse, TokenInfoResponse};
use cw_multi_test::{next_block, App, Executor};
use dao_interface::voting::{InfoResponse, IsActiveResponse, VotingPowerAtHeightResponse};
use dao_testing::contracts::{
    cw20_base_contract, cw20_stake_contract, dao_voting_cw20_staked_contract,
};
use dao_voting::threshold::{ActiveThreshold, ActiveThresholdResponse};

use crate::{
    contract::{migrate, CONTRACT_NAME, CONTRACT_VERSION},
    msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, StakingInfo},
};

const DAO_ADDR: &str = "dao";
const CREATOR_ADDR: &str = "creator";

fn instantiate_voting(app: &mut App, voting_id: u64, msg: InstantiateMsg) -> Addr {
    app.instantiate_contract(
        voting_id,
        Addr::unchecked(DAO_ADDR),
        &msg,
        &[],
        "voting module",
        None,
    )
    .unwrap()
}

fn stake_tokens(app: &mut App, staking_addr: Addr, cw20_addr: Addr, sender: &str, amount: u128) {
    let msg = cw20::Cw20ExecuteMsg::Send {
        contract: staking_addr.to_string(),
        amount: Uint128::new(amount),
        msg: to_json_binary(&cw20_stake::msg::ReceiveMsg::Stake {}).unwrap(),
    };
    app.execute_contract(Addr::unchecked(sender), cw20_addr, &msg, &[])
        .unwrap();
}

#[test]
#[should_panic(expected = "Initial governance token balances must not be empty")]
fn test_instantiate_zero_supply() {
    let mut app = App::default();
    let cw20_id = app.store_code(cw20_base_contract());
    let voting_id = app.store_code(dao_voting_cw20_staked_contract());
    let staking_contract_id = app.store_code(cw20_stake_contract());
    instantiate_voting(
        &mut app,
        voting_id,
        InstantiateMsg {
            token_info: crate::msg::TokenInfo::New {
                code_id: cw20_id,
                label: "DAO DAO voting".to_string(),
                name: "DAO DAO".to_string(),
                symbol: "DAO".to_string(),
                decimals: 6,
                initial_balances: vec![Cw20Coin {
                    address: CREATOR_ADDR.to_string(),
                    amount: Uint128::zero(),
                }],
                marketing: None,
                unstaking_duration: None,
                staking_code_id: staking_contract_id,
                initial_dao_balance: Some(Uint128::zero()),
            },
            active_threshold: None,
        },
    );
}

#[test]
#[should_panic(expected = "Initial governance token balances must not be empty")]
fn test_instantiate_no_balances() {
    let mut app = App::default();
    let cw20_id = app.store_code(cw20_base_contract());
    let voting_id = app.store_code(dao_voting_cw20_staked_contract());
    let staking_contract_id = app.store_code(cw20_stake_contract());
    instantiate_voting(
        &mut app,
        voting_id,
        InstantiateMsg {
            token_info: crate::msg::TokenInfo::New {
                code_id: cw20_id,
                label: "DAO DAO voting".to_string(),
                name: "DAO DAO".to_string(),
                symbol: "DAO".to_string(),
                decimals: 6,
                initial_balances: vec![],
                marketing: None,
                unstaking_duration: None,
                staking_code_id: staking_contract_id,
                initial_dao_balance: Some(Uint128::zero()),
            },
            active_threshold: None,
        },
    );
}

#[test]
#[should_panic(expected = "Active threshold count must be greater than zero")]
fn test_instantiate_zero_active_threshold_count() {
    let mut app = App::default();
    let cw20_id = app.store_code(cw20_base_contract());
    let voting_id = app.store_code(dao_voting_cw20_staked_contract());
    let staking_contract_id = app.store_code(cw20_stake_contract());
    instantiate_voting(
        &mut app,
        voting_id,
        InstantiateMsg {
            token_info: crate::msg::TokenInfo::New {
                code_id: cw20_id,
                label: "DAO DAO voting".to_string(),
                name: "DAO DAO".to_string(),
                symbol: "DAO".to_string(),
                decimals: 6,
                initial_balances: vec![Cw20Coin {
                    address: CREATOR_ADDR.to_string(),
                    amount: Uint128::one(),
                }],
                marketing: None,
                unstaking_duration: None,
                staking_code_id: staking_contract_id,
                initial_dao_balance: Some(Uint128::zero()),
            },
            active_threshold: Some(ActiveThreshold::AbsoluteCount {
                count: Uint128::new(0),
            }),
        },
    );
}

#[test]
fn test_contract_info() {
    let mut app = App::default();
    let cw20_id = app.store_code(cw20_base_contract());
    let voting_id = app.store_code(dao_voting_cw20_staked_contract());
    let staking_contract_id = app.store_code(cw20_stake_contract());

    let voting_addr = instantiate_voting(
        &mut app,
        voting_id,
        InstantiateMsg {
            token_info: crate::msg::TokenInfo::New {
                code_id: cw20_id,
                label: "DAO DAO voting".to_string(),
                name: "DAO DAO".to_string(),
                symbol: "DAO".to_string(),
                decimals: 6,
                initial_balances: vec![Cw20Coin {
                    address: CREATOR_ADDR.to_string(),
                    amount: Uint128::from(2u64),
                }],
                marketing: None,
                unstaking_duration: None,
                staking_code_id: staking_contract_id,
                initial_dao_balance: Some(Uint128::zero()),
            },
            active_threshold: None,
        },
    );

    let info: InfoResponse = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::Info {})
        .unwrap();
    assert_eq!(
        info,
        InfoResponse {
            info: ContractVersion {
                contract: "crates.io:dao-voting-cw20-staked".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string()
            }
        }
    );

    let dao: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr, &QueryMsg::Dao {})
        .unwrap();
    assert_eq!(dao, Addr::unchecked(DAO_ADDR));
}

#[test]
fn test_new_cw20() {
    let mut app = App::default();
    let cw20_id = app.store_code(cw20_base_contract());
    let voting_id = app.store_code(dao_voting_cw20_staked_contract());
    let staking_contract_id = app.store_code(cw20_stake_contract());

    let voting_addr = instantiate_voting(
        &mut app,
        voting_id,
        InstantiateMsg {
            token_info: crate::msg::TokenInfo::New {
                code_id: cw20_id,
                label: "DAO DAO voting".to_string(),
                name: "DAO DAO".to_string(),
                symbol: "DAO".to_string(),
                decimals: 6,
                initial_balances: vec![Cw20Coin {
                    address: CREATOR_ADDR.to_string(),
                    amount: Uint128::from(2u64),
                }],
                marketing: None,
                unstaking_duration: None,
                staking_code_id: staking_contract_id,
                initial_dao_balance: Some(Uint128::from(10u64)),
            },
            active_threshold: None,
        },
    );

    let token_addr: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::TokenContract {})
        .unwrap();
    let staking_addr: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::StakingContract {})
        .unwrap();

    let token_info: TokenInfoResponse = app
        .wrap()
        .query_wasm_smart(token_addr.clone(), &cw20::Cw20QueryMsg::TokenInfo {})
        .unwrap();
    assert_eq!(
        token_info,
        TokenInfoResponse {
            name: "DAO DAO".to_string(),
            symbol: "DAO".to_string(),
            decimals: 6,
            total_supply: Uint128::from(12u64)
        }
    );

    let minter_info: Option<MinterResponse> = app
        .wrap()
        .query_wasm_smart(token_addr.clone(), &cw20::Cw20QueryMsg::Minter {})
        .unwrap();
    assert_eq!(
        minter_info,
        Some(MinterResponse {
            minter: DAO_ADDR.to_string(),
            cap: None,
        })
    );

    // Expect DAO (sender address) to have initial balance.
    let token_info: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            token_addr.clone(),
            &cw20::Cw20QueryMsg::Balance {
                address: DAO_ADDR.to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        token_info,
        BalanceResponse {
            balance: Uint128::from(10u64)
        }
    );

    // Expect 0 as they have not staked
    let creator_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::VotingPowerAtHeight {
                address: CREATOR_ADDR.to_string(),
                height: None,
            },
        )
        .unwrap();

    assert_eq!(
        creator_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::zero(),
            height: app.block_info().height,
        }
    );

    // Expect 0 as DAO has not staked
    let dao_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::VotingPowerAtHeight {
                address: DAO_ADDR.to_string(),
                height: None,
            },
        )
        .unwrap();

    assert_eq!(
        dao_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::zero(),
            height: app.block_info().height,
        }
    );

    // Stake 1 token as creator
    stake_tokens(&mut app, staking_addr, token_addr, CREATOR_ADDR, 1);
    app.update_block(next_block);

    // Expect 1 as creator has now staked 1
    let creator_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::VotingPowerAtHeight {
                address: CREATOR_ADDR.to_string(),
                height: None,
            },
        )
        .unwrap();

    assert_eq!(
        creator_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::new(1u128),
            height: app.block_info().height,
        }
    );

    // Expect 1 as only one token staked to make up whole voting power
    let total_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(voting_addr, &QueryMsg::TotalPowerAtHeight { height: None })
        .unwrap();

    assert_eq!(
        total_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::new(1u128),
            height: app.block_info().height,
        }
    )
}

#[test]
fn test_existing_cw20_new_staking() {
    let mut app = App::default();
    let cw20_id = app.store_code(cw20_base_contract());
    let voting_id = app.store_code(dao_voting_cw20_staked_contract());
    let staking_id = app.store_code(cw20_stake_contract());

    let token_addr = app
        .instantiate_contract(
            cw20_id,
            Addr::unchecked(CREATOR_ADDR),
            &cw20_base::msg::InstantiateMsg {
                name: "DAO DAO".to_string(),
                symbol: "DAO".to_string(),
                decimals: 3,
                initial_balances: vec![Cw20Coin {
                    address: CREATOR_ADDR.to_string(),
                    amount: Uint128::from(2u64),
                }],
                mint: None,
                marketing: None,
            },
            &[],
            "voting token",
            None,
        )
        .unwrap();

    let voting_addr = instantiate_voting(
        &mut app,
        voting_id,
        InstantiateMsg {
            token_info: crate::msg::TokenInfo::Existing {
                address: token_addr.to_string(),
                staking_contract: StakingInfo::New {
                    staking_code_id: staking_id,
                    unstaking_duration: None,
                },
            },
            active_threshold: None,
        },
    );

    let token_addr: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::TokenContract {})
        .unwrap();
    let staking_addr: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::StakingContract {})
        .unwrap();

    let token_info: TokenInfoResponse = app
        .wrap()
        .query_wasm_smart(token_addr.clone(), &cw20::Cw20QueryMsg::TokenInfo {})
        .unwrap();
    assert_eq!(
        token_info,
        TokenInfoResponse {
            name: "DAO DAO".to_string(),
            symbol: "DAO".to_string(),
            decimals: 3,
            total_supply: Uint128::from(2u64)
        }
    );

    let minter_info: Option<MinterResponse> = app
        .wrap()
        .query_wasm_smart(token_addr.clone(), &cw20::Cw20QueryMsg::Minter {})
        .unwrap();
    assert!(minter_info.is_none());

    // Expect 0 as creator has not staked
    let creator_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::VotingPowerAtHeight {
                address: CREATOR_ADDR.to_string(),
                height: None,
            },
        )
        .unwrap();

    assert_eq!(
        creator_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::zero(),
            height: app.block_info().height,
        }
    );

    // Expect 0 as DAO has not staked
    let dao_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::VotingPowerAtHeight {
                address: DAO_ADDR.to_string(),
                height: None,
            },
        )
        .unwrap();

    assert_eq!(
        dao_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::zero(),
            height: app.block_info().height,
        }
    );

    // Stake 1 token as creator
    stake_tokens(&mut app, staking_addr, token_addr, CREATOR_ADDR, 1);
    app.update_block(next_block);

    // Expect 1 as creator has now staked 1
    let creator_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::VotingPowerAtHeight {
                address: CREATOR_ADDR.to_string(),
                height: None,
            },
        )
        .unwrap();

    assert_eq!(
        creator_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::new(1u128),
            height: app.block_info().height,
        }
    );

    // Expect 1 as only one token staked to make up whole voting power
    let total_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(voting_addr, &QueryMsg::TotalPowerAtHeight { height: None })
        .unwrap();

    assert_eq!(
        total_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::new(1u128),
            height: app.block_info().height,
        }
    )
}

#[test]
fn test_existing_cw20_existing_staking() {
    let mut app = App::default();
    let cw20_id = app.store_code(cw20_base_contract());
    let voting_id = app.store_code(dao_voting_cw20_staked_contract());
    let staking_id = app.store_code(cw20_stake_contract());

    let token_addr = app
        .instantiate_contract(
            cw20_id,
            Addr::unchecked(CREATOR_ADDR),
            &cw20_base::msg::InstantiateMsg {
                name: "DAO DAO".to_string(),
                symbol: "DAO".to_string(),
                decimals: 3,
                initial_balances: vec![Cw20Coin {
                    address: CREATOR_ADDR.to_string(),
                    amount: Uint128::from(2u64),
                }],
                mint: None,
                marketing: None,
            },
            &[],
            "voting token",
            None,
        )
        .unwrap();

    let voting_addr = instantiate_voting(
        &mut app,
        voting_id,
        InstantiateMsg {
            token_info: crate::msg::TokenInfo::Existing {
                address: token_addr.to_string(),
                staking_contract: StakingInfo::New {
                    staking_code_id: staking_id,
                    unstaking_duration: None,
                },
            },
            active_threshold: None,
        },
    );

    let token_addr: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::TokenContract {})
        .unwrap();
    // We'll use this for our valid existing contract
    let staking_addr: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr, &QueryMsg::StakingContract {})
        .unwrap();

    let token_info: TokenInfoResponse = app
        .wrap()
        .query_wasm_smart(token_addr.clone(), &cw20::Cw20QueryMsg::TokenInfo {})
        .unwrap();
    assert_eq!(
        token_info,
        TokenInfoResponse {
            name: "DAO DAO".to_string(),
            symbol: "DAO".to_string(),
            decimals: 3,
            total_supply: Uint128::from(2u64)
        }
    );

    let voting_addr = instantiate_voting(
        &mut app,
        voting_id,
        InstantiateMsg {
            token_info: crate::msg::TokenInfo::Existing {
                address: token_addr.to_string(),
                staking_contract: StakingInfo::Existing {
                    staking_contract_address: staking_addr.to_string(),
                },
            },
            active_threshold: None,
        },
    );

    // Expect 0 as creator has not staked
    let creator_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::VotingPowerAtHeight {
                address: CREATOR_ADDR.to_string(),
                height: None,
            },
        )
        .unwrap();

    assert_eq!(
        creator_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::zero(),
            height: app.block_info().height,
        }
    );

    // Expect 0 as DAO has not staked
    let dao_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::VotingPowerAtHeight {
                address: DAO_ADDR.to_string(),
                height: None,
            },
        )
        .unwrap();

    assert_eq!(
        dao_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::zero(),
            height: app.block_info().height,
        }
    );

    // Stake 1 token as creator
    stake_tokens(&mut app, staking_addr.clone(), token_addr, CREATOR_ADDR, 1);
    app.update_block(next_block);

    // Expect 1 as creator has now staked 1
    let creator_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::VotingPowerAtHeight {
                address: CREATOR_ADDR.to_string(),
                height: None,
            },
        )
        .unwrap();

    assert_eq!(
        creator_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::new(1u128),
            height: app.block_info().height,
        }
    );

    // Expect 1 as only one token staked to make up whole voting power
    let total_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(voting_addr, &QueryMsg::TotalPowerAtHeight { height: None })
        .unwrap();

    assert_eq!(
        total_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::new(1u128),
            height: app.block_info().height,
        }
    );

    // Now lets test the error case where we use an invalid staking contract
    let different_token = app
        .instantiate_contract(
            cw20_id,
            Addr::unchecked(CREATOR_ADDR),
            &cw20_base::msg::InstantiateMsg {
                name: "DAO DAO MISMATCH".to_string(),
                symbol: "DAOM".to_string(),
                decimals: 3,
                initial_balances: vec![Cw20Coin {
                    address: CREATOR_ADDR.to_string(),
                    amount: Uint128::from(2u64),
                }],
                mint: None,
                marketing: None,
            },
            &[],
            "voting token",
            None,
        )
        .unwrap();

    // Expect error as the token address does not match the staking address token address
    app.instantiate_contract(
        voting_id,
        Addr::unchecked(DAO_ADDR),
        &InstantiateMsg {
            token_info: crate::msg::TokenInfo::Existing {
                address: different_token.to_string(),
                staking_contract: StakingInfo::Existing {
                    staking_contract_address: staking_addr.to_string(),
                },
            },
            active_threshold: None,
        },
        &[],
        "voting module",
        None,
    )
    .unwrap_err();
}

#[test]
fn test_different_heights() {
    let mut app = App::default();
    let cw20_id = app.store_code(cw20_base_contract());
    let voting_id = app.store_code(dao_voting_cw20_staked_contract());
    let staking_id = app.store_code(cw20_stake_contract());

    let token_addr = app
        .instantiate_contract(
            cw20_id,
            Addr::unchecked(CREATOR_ADDR),
            &cw20_base::msg::InstantiateMsg {
                name: "DAO DAO".to_string(),
                symbol: "DAO".to_string(),
                decimals: 3,
                initial_balances: vec![Cw20Coin {
                    address: CREATOR_ADDR.to_string(),
                    amount: Uint128::from(2u64),
                }],
                mint: None,
                marketing: None,
            },
            &[],
            "voting token",
            None,
        )
        .unwrap();

    let voting_addr = instantiate_voting(
        &mut app,
        voting_id,
        InstantiateMsg {
            token_info: crate::msg::TokenInfo::Existing {
                address: token_addr.to_string(),
                staking_contract: StakingInfo::New {
                    staking_code_id: staking_id,
                    unstaking_duration: None,
                },
            },
            active_threshold: None,
        },
    );

    let token_addr: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::TokenContract {})
        .unwrap();
    let staking_addr: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::StakingContract {})
        .unwrap();

    // Expect 0 as creator has not staked
    let creator_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::VotingPowerAtHeight {
                address: CREATOR_ADDR.to_string(),
                height: None,
            },
        )
        .unwrap();

    assert_eq!(
        creator_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::zero(),
            height: app.block_info().height,
        }
    );

    // Stake 1 token as creator
    stake_tokens(
        &mut app,
        staking_addr.clone(),
        token_addr.clone(),
        CREATOR_ADDR,
        1,
    );
    app.update_block(next_block);

    // Expect 1 as creator has now staked 1
    let creator_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::VotingPowerAtHeight {
                address: CREATOR_ADDR.to_string(),
                height: None,
            },
        )
        .unwrap();

    assert_eq!(
        creator_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::new(1u128),
            height: app.block_info().height,
        }
    );

    // Expect 1 as only one token staked to make up whole voting power
    let total_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::TotalPowerAtHeight { height: None },
        )
        .unwrap();

    assert_eq!(
        total_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::new(1u128),
            height: app.block_info().height,
        }
    );

    // Stake another 1 token as creator
    stake_tokens(&mut app, staking_addr, token_addr, CREATOR_ADDR, 1);
    app.update_block(next_block);

    // Expect 2 as creator has now staked 2
    let creator_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::VotingPowerAtHeight {
                address: CREATOR_ADDR.to_string(),
                height: None,
            },
        )
        .unwrap();

    assert_eq!(
        creator_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::new(2u128),
            height: app.block_info().height,
        }
    );

    // Expect 2 as we have now staked 2
    let total_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::TotalPowerAtHeight { height: None },
        )
        .unwrap();

    assert_eq!(
        total_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::new(2u128),
            height: app.block_info().height,
        }
    );

    // Check we can query history
    let creator_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::VotingPowerAtHeight {
                address: CREATOR_ADDR.to_string(),
                height: Some(app.block_info().height - 1),
            },
        )
        .unwrap();

    assert_eq!(
        creator_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::new(1u128),
            height: app.block_info().height - 1,
        }
    );

    // Expect 1 at the old height prior to second stake
    let total_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr,
            &QueryMsg::TotalPowerAtHeight {
                height: Some(app.block_info().height - 1),
            },
        )
        .unwrap();

    assert_eq!(
        total_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::new(1u128),
            height: app.block_info().height - 1,
        }
    );
}

#[test]
fn test_active_threshold_absolute_count() {
    let mut app = App::default();
    let cw20_id = app.store_code(cw20_base_contract());
    let voting_id = app.store_code(dao_voting_cw20_staked_contract());
    let staking_contract_id = app.store_code(cw20_stake_contract());

    let voting_addr = instantiate_voting(
        &mut app,
        voting_id,
        InstantiateMsg {
            token_info: crate::msg::TokenInfo::New {
                code_id: cw20_id,
                label: "DAO DAO voting".to_string(),
                name: "DAO DAO".to_string(),
                symbol: "DAO".to_string(),
                decimals: 6,
                initial_balances: vec![Cw20Coin {
                    address: CREATOR_ADDR.to_string(),
                    amount: Uint128::from(200u64),
                }],
                marketing: None,
                unstaking_duration: None,
                staking_code_id: staking_contract_id,
                initial_dao_balance: Some(Uint128::from(100u64)),
            },
            active_threshold: Some(ActiveThreshold::AbsoluteCount {
                count: Uint128::new(100),
            }),
        },
    );

    let token_addr: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::TokenContract {})
        .unwrap();
    let staking_addr: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::StakingContract {})
        .unwrap();

    // Not active as none staked
    let is_active: IsActiveResponse = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::IsActive {})
        .unwrap();
    assert!(!is_active.active);

    // Stake 100 token as creator
    stake_tokens(&mut app, staking_addr, token_addr, CREATOR_ADDR, 100);
    app.update_block(next_block);

    // Active as enough staked
    let is_active: IsActiveResponse = app
        .wrap()
        .query_wasm_smart(voting_addr, &QueryMsg::IsActive {})
        .unwrap();
    assert!(is_active.active);
}

#[test]
fn test_active_threshold_percent() {
    let mut app = App::default();
    let cw20_id = app.store_code(cw20_base_contract());
    let voting_id = app.store_code(dao_voting_cw20_staked_contract());
    let staking_contract_id = app.store_code(cw20_stake_contract());

    let voting_addr = instantiate_voting(
        &mut app,
        voting_id,
        InstantiateMsg {
            token_info: crate::msg::TokenInfo::New {
                code_id: cw20_id,
                label: "DAO DAO voting".to_string(),
                name: "DAO DAO".to_string(),
                symbol: "DAO".to_string(),
                decimals: 6,
                initial_balances: vec![Cw20Coin {
                    address: CREATOR_ADDR.to_string(),
                    amount: Uint128::from(200u64),
                }],
                marketing: None,
                unstaking_duration: None,
                staking_code_id: staking_contract_id,
                initial_dao_balance: Some(Uint128::from(100u64)),
            },
            active_threshold: Some(ActiveThreshold::Percentage {
                percent: Decimal::percent(20),
            }),
        },
    );

    let token_addr: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::TokenContract {})
        .unwrap();
    let staking_addr: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::StakingContract {})
        .unwrap();

    // Not active as none staked
    let is_active: IsActiveResponse = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::IsActive {})
        .unwrap();
    assert!(!is_active.active);

    // Stake 60 token as creator, now active
    stake_tokens(&mut app, staking_addr, token_addr, CREATOR_ADDR, 60);
    app.update_block(next_block);

    // Active as enough staked
    let is_active: IsActiveResponse = app
        .wrap()
        .query_wasm_smart(voting_addr, &QueryMsg::IsActive {})
        .unwrap();
    assert!(is_active.active);
}

#[test]
fn test_active_threshold_percent_rounds_up() {
    let mut app = App::default();
    let cw20_id = app.store_code(cw20_base_contract());
    let voting_id = app.store_code(dao_voting_cw20_staked_contract());
    let staking_contract_id = app.store_code(cw20_stake_contract());

    let voting_addr = instantiate_voting(
        &mut app,
        voting_id,
        InstantiateMsg {
            token_info: crate::msg::TokenInfo::New {
                code_id: cw20_id,
                label: "DAO DAO voting".to_string(),
                name: "DAO DAO".to_string(),
                symbol: "DAO".to_string(),
                decimals: 6,
                initial_balances: vec![Cw20Coin {
                    address: CREATOR_ADDR.to_string(),
                    amount: Uint128::from(5u64),
                }],
                marketing: None,
                unstaking_duration: None,
                staking_code_id: staking_contract_id,
                initial_dao_balance: None,
            },
            active_threshold: Some(ActiveThreshold::Percentage {
                percent: Decimal::percent(50),
            }),
        },
    );

    let token_addr: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::TokenContract {})
        .unwrap();
    let staking_addr: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::StakingContract {})
        .unwrap();

    // Not active as none staked
    let is_active: IsActiveResponse = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::IsActive {})
        .unwrap();
    assert!(!is_active.active);

    // Stake 2 token as creator, should not be active.
    stake_tokens(
        &mut app,
        staking_addr.clone(),
        token_addr.clone(),
        CREATOR_ADDR,
        2,
    );
    app.update_block(next_block);

    let is_active: IsActiveResponse = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::IsActive {})
        .unwrap();
    assert!(!is_active.active);

    // Stake 1 more token as creator, should now be active.
    stake_tokens(&mut app, staking_addr, token_addr, CREATOR_ADDR, 1);
    app.update_block(next_block);

    let is_active: IsActiveResponse = app
        .wrap()
        .query_wasm_smart(voting_addr, &QueryMsg::IsActive {})
        .unwrap();
    assert!(is_active.active);
}

#[test]
fn test_active_threshold_none() {
    let mut app = App::default();
    let cw20_id = app.store_code(cw20_base_contract());
    let voting_id = app.store_code(dao_voting_cw20_staked_contract());
    let staking_contract_id = app.store_code(cw20_stake_contract());

    let voting_addr = instantiate_voting(
        &mut app,
        voting_id,
        InstantiateMsg {
            token_info: crate::msg::TokenInfo::New {
                code_id: cw20_id,
                label: "DAO DAO voting".to_string(),
                name: "DAO DAO".to_string(),
                symbol: "DAO".to_string(),
                decimals: 6,
                initial_balances: vec![Cw20Coin {
                    address: CREATOR_ADDR.to_string(),
                    amount: Uint128::from(200u64),
                }],
                marketing: None,
                unstaking_duration: None,
                staking_code_id: staking_contract_id,
                initial_dao_balance: Some(Uint128::from(100u64)),
            },
            active_threshold: None,
        },
    );

    // Active as no threshold
    let is_active: IsActiveResponse = app
        .wrap()
        .query_wasm_smart(voting_addr, &QueryMsg::IsActive {})
        .unwrap();
    assert!(is_active.active);
}

#[test]
fn test_update_active_threshold() {
    let mut app = App::default();
    let cw20_id = app.store_code(cw20_base_contract());
    let voting_id = app.store_code(dao_voting_cw20_staked_contract());
    let staking_contract_id = app.store_code(cw20_stake_contract());

    let voting_addr = instantiate_voting(
        &mut app,
        voting_id,
        InstantiateMsg {
            token_info: crate::msg::TokenInfo::New {
                code_id: cw20_id,
                label: "DAO DAO voting".to_string(),
                name: "DAO DAO".to_string(),
                symbol: "DAO".to_string(),
                decimals: 6,
                initial_balances: vec![Cw20Coin {
                    address: CREATOR_ADDR.to_string(),
                    amount: Uint128::from(200u64),
                }],
                marketing: None,
                unstaking_duration: None,
                staking_code_id: staking_contract_id,
                initial_dao_balance: Some(Uint128::from(100u64)),
            },
            active_threshold: None,
        },
    );

    let resp: ActiveThresholdResponse = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::ActiveThreshold {})
        .unwrap();
    assert_eq!(resp.active_threshold, None);

    let msg = ExecuteMsg::UpdateActiveThreshold {
        new_threshold: Some(ActiveThreshold::AbsoluteCount {
            count: Uint128::new(100),
        }),
    };

    // Expect failure as sender is not the DAO
    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        voting_addr.clone(),
        &msg,
        &[],
    )
    .unwrap_err();

    // Expect success as sender is the DAO
    app.execute_contract(Addr::unchecked(DAO_ADDR), voting_addr.clone(), &msg, &[])
        .unwrap();

    let resp: ActiveThresholdResponse = app
        .wrap()
        .query_wasm_smart(voting_addr, &QueryMsg::ActiveThreshold {})
        .unwrap();
    assert_eq!(
        resp.active_threshold,
        Some(ActiveThreshold::AbsoluteCount {
            count: Uint128::new(100)
        })
    );
}

#[test]
#[should_panic(expected = "Active threshold percentage must be greater than 0 and less than 1")]
fn test_active_threshold_percentage_gt_100() {
    let mut app = App::default();
    let cw20_id = app.store_code(cw20_base_contract());
    let voting_id = app.store_code(dao_voting_cw20_staked_contract());
    let staking_contract_id = app.store_code(cw20_stake_contract());

    instantiate_voting(
        &mut app,
        voting_id,
        InstantiateMsg {
            token_info: crate::msg::TokenInfo::New {
                code_id: cw20_id,
                label: "DAO DAO voting".to_string(),
                name: "DAO DAO".to_string(),
                symbol: "DAO".to_string(),
                decimals: 6,
                initial_balances: vec![Cw20Coin {
                    address: CREATOR_ADDR.to_string(),
                    amount: Uint128::from(200u64),
                }],
                marketing: None,
                unstaking_duration: None,
                staking_code_id: staking_contract_id,
                initial_dao_balance: Some(Uint128::from(100u64)),
            },
            active_threshold: Some(ActiveThreshold::Percentage {
                percent: Decimal::percent(120),
            }),
        },
    );
}

#[test]
#[should_panic(expected = "Active threshold percentage must be greater than 0 and less than 1")]
fn test_active_threshold_percentage_lte_0() {
    let mut app = App::default();
    let cw20_id = app.store_code(cw20_base_contract());
    let voting_id = app.store_code(dao_voting_cw20_staked_contract());
    let staking_contract_id = app.store_code(cw20_stake_contract());

    instantiate_voting(
        &mut app,
        voting_id,
        InstantiateMsg {
            token_info: crate::msg::TokenInfo::New {
                code_id: cw20_id,
                label: "DAO DAO voting".to_string(),
                name: "DAO DAO".to_string(),
                symbol: "DAO".to_string(),
                decimals: 6,
                initial_balances: vec![Cw20Coin {
                    address: CREATOR_ADDR.to_string(),
                    amount: Uint128::from(200u64),
                }],
                marketing: None,
                unstaking_duration: None,
                staking_code_id: staking_contract_id,
                initial_dao_balance: Some(Uint128::from(100u64)),
            },
            active_threshold: Some(ActiveThreshold::Percentage {
                percent: Decimal::percent(0),
            }),
        },
    );
}

#[test]
#[should_panic(expected = "Absolute count threshold cannot be greater than the total token supply")]
fn test_active_threshold_absolute_count_invalid() {
    let mut app = App::default();
    let cw20_id = app.store_code(cw20_base_contract());
    let voting_id = app.store_code(dao_voting_cw20_staked_contract());
    let staking_contract_id = app.store_code(cw20_stake_contract());

    instantiate_voting(
        &mut app,
        voting_id,
        InstantiateMsg {
            token_info: crate::msg::TokenInfo::New {
                code_id: cw20_id,
                label: "DAO DAO voting".to_string(),
                name: "DAO DAO".to_string(),
                symbol: "DAO".to_string(),
                decimals: 6,
                initial_balances: vec![Cw20Coin {
                    address: CREATOR_ADDR.to_string(),
                    amount: Uint128::from(200u64),
                }],
                marketing: None,
                unstaking_duration: None,
                staking_code_id: staking_contract_id,
                initial_dao_balance: Some(Uint128::from(100u64)),
            },
            active_threshold: Some(ActiveThreshold::AbsoluteCount {
                count: Uint128::new(10000),
            }),
        },
    );
}

#[test]
fn test_migrate() {
    let mut app = App::default();
    let cw20_id = app.store_code(cw20_base_contract());
    let voting_id = app.store_code(dao_voting_cw20_staked_contract());
    let staking_contract_id = app.store_code(cw20_stake_contract());

    let voting_addr = app
        .instantiate_contract(
            voting_id,
            Addr::unchecked(DAO_ADDR),
            &InstantiateMsg {
                token_info: crate::msg::TokenInfo::New {
                    code_id: cw20_id,
                    label: "DAO DAO voting".to_string(),
                    name: "DAO DAO".to_string(),
                    symbol: "DAO".to_string(),
                    decimals: 6,
                    initial_balances: vec![Cw20Coin {
                        address: CREATOR_ADDR.to_string(),
                        amount: Uint128::from(2u64),
                    }],
                    marketing: None,
                    unstaking_duration: None,
                    staking_code_id: staking_contract_id,
                    initial_dao_balance: Some(Uint128::zero()),
                },
                active_threshold: None,
            },
            &[],
            "voting module",
            Some(DAO_ADDR.to_string()),
        )
        .unwrap();

    let info: InfoResponse = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::Info {})
        .unwrap();

    let dao: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::Dao {})
        .unwrap();

    app.execute(
        dao,
        CosmosMsg::Wasm(WasmMsg::Migrate {
            contract_addr: voting_addr.to_string(),
            new_code_id: voting_id,
            msg: to_json_binary(&MigrateMsg {}).unwrap(),
        }),
    )
    .unwrap();

    let new_info: InfoResponse = app
        .wrap()
        .query_wasm_smart(voting_addr, &QueryMsg::Info {})
        .unwrap();

    assert_eq!(info, new_info);
}

#[test]
pub fn test_migrate_update_version() {
    let mut deps = mock_dependencies();
    cw2::set_contract_version(&mut deps.storage, "my-contract", "1.0.0").unwrap();
    migrate(deps.as_mut(), mock_env(), MigrateMsg {}).unwrap();
    let version = cw2::get_contract_version(&deps.storage).unwrap();
    assert_eq!(version.version, CONTRACT_VERSION);
    assert_eq!(version.contract, CONTRACT_NAME);
}
