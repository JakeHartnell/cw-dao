use std::borrow::BorrowMut;

use cosmwasm_std::{coin, to_json_binary, Addr, Uint128, WasmMsg};
use cw20::{Cw20Coin, Cw20ExecuteMsg, Denom};
use cw20_stake_external_rewards_v1 as v1;
use cw_multi_test::{next_block, App, BankSudo, Executor, SudoMsg};
use cw_ownable::{Action, Ownership, OwnershipError};
use cw_utils::Duration;
use dao_testing::contracts::{
    cw20_base_contract, cw20_stake_contract, cw20_stake_external_rewards_contract,
    v1::cw20_stake_external_rewards_v1_contract,
};

use crate::msg::{
    ExecuteMsg, InfoResponse, MigrateMsg, PendingRewardsResponse, QueryMsg, ReceiveMsg,
};
use cw20_stake_external_rewards::ContractError;

const OWNER: &str = "owner";
const ADDR1: &str = "addr0001";
const ADDR2: &str = "addr0002";
const ADDR3: &str = "addr0003";

fn mock_app() -> App {
    App::default()
}

fn instantiate_cw20(app: &mut App, initial_balances: Vec<Cw20Coin>) -> Addr {
    let cw20_id = app.store_code(cw20_base_contract());
    let msg = cw20_base::msg::InstantiateMsg {
        name: String::from("Test"),
        symbol: String::from("TEST"),
        decimals: 6,
        initial_balances,
        mint: None,
        marketing: None,
    };

    app.instantiate_contract(cw20_id, Addr::unchecked(ADDR1), &msg, &[], "cw20", None)
        .unwrap()
}

fn instantiate_staking(app: &mut App, cw20: Addr, unstaking_duration: Option<Duration>) -> Addr {
    let staking_code_id = app.store_code(cw20_stake_contract());
    let msg = cw20_stake::msg::InstantiateMsg {
        owner: Some(OWNER.to_string()),
        token_address: cw20.to_string(),
        unstaking_duration,
    };
    app.instantiate_contract(
        staking_code_id,
        Addr::unchecked(ADDR1),
        &msg,
        &[],
        "staking",
        None,
    )
    .unwrap()
}

fn stake_tokens<T: Into<String>>(
    app: &mut App,
    staking_addr: &Addr,
    cw20_addr: &Addr,
    sender: T,
    amount: u128,
) {
    let msg = cw20::Cw20ExecuteMsg::Send {
        contract: staking_addr.to_string(),
        amount: Uint128::new(amount),
        msg: to_json_binary(&cw20_stake::msg::ReceiveMsg::Stake {}).unwrap(),
    };
    app.execute_contract(Addr::unchecked(sender), cw20_addr.clone(), &msg, &[])
        .unwrap();
}

fn unstake_tokens(app: &mut App, staking_addr: &Addr, address: &str, amount: u128) {
    let msg = cw20_stake::msg::ExecuteMsg::Unstake {
        amount: Uint128::new(amount),
    };
    app.execute_contract(Addr::unchecked(address), staking_addr.clone(), &msg, &[])
        .unwrap();
}

fn setup_staking_contract(app: &mut App, initial_balances: Vec<Cw20Coin>) -> (Addr, Addr) {
    // Instantiate cw20 contract
    let cw20_addr = instantiate_cw20(app, initial_balances.clone());
    app.update_block(next_block);
    // Instantiate staking contract
    let staking_addr = instantiate_staking(app, cw20_addr.clone(), None);
    app.update_block(next_block);
    for coin in initial_balances {
        stake_tokens(
            app,
            &staking_addr,
            &cw20_addr,
            coin.address,
            coin.amount.u128(),
        );
    }
    (staking_addr, cw20_addr)
}

fn setup_reward_contract(
    app: &mut App,
    staking_contract: Addr,
    reward_token: Denom,
    owner: Addr,
) -> Addr {
    let reward_code_id = app.store_code(cw20_stake_external_rewards_contract());
    let msg = crate::msg::InstantiateMsg {
        owner: Some(owner.clone().into_string()),
        staking_contract: staking_contract.clone().into_string(),
        reward_token,
        reward_duration: 100000,
    };
    let reward_addr = app
        .instantiate_contract(reward_code_id, owner, &msg, &[], "reward", None)
        .unwrap();
    let msg = cw20_stake::msg::ExecuteMsg::AddHook {
        addr: reward_addr.to_string(),
    };
    let _result = app
        .execute_contract(Addr::unchecked(OWNER), staking_contract, &msg, &[])
        .unwrap();
    reward_addr
}

fn get_balance_cw20<T: Into<String>, U: Into<String>>(
    app: &App,
    contract_addr: T,
    address: U,
) -> Uint128 {
    let msg = cw20::Cw20QueryMsg::Balance {
        address: address.into(),
    };
    let result: cw20::BalanceResponse = app.wrap().query_wasm_smart(contract_addr, &msg).unwrap();
    result.balance
}

fn get_balance_native<T: Into<String>, U: Into<String>>(
    app: &App,
    address: T,
    denom: U,
) -> Uint128 {
    app.wrap().query_balance(address, denom).unwrap().amount
}

fn get_ownership<T: Into<String>>(app: &App, address: T) -> Ownership<Addr> {
    app.wrap()
        .query_wasm_smart(address, &QueryMsg::Ownership {})
        .unwrap()
}

fn assert_pending_rewards(app: &mut App, reward_addr: &Addr, address: &str, expected: u128) {
    let res: PendingRewardsResponse = app
        .borrow_mut()
        .wrap()
        .query_wasm_smart(
            reward_addr,
            &QueryMsg::GetPendingRewards {
                address: address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(res.pending_rewards, Uint128::new(expected));
}

fn claim_rewards(app: &mut App, reward_addr: Addr, address: &str) {
    let msg = ExecuteMsg::Claim {};
    app.borrow_mut()
        .execute_contract(Addr::unchecked(address), reward_addr, &msg, &[])
        .unwrap();
}

fn fund_rewards_cw20(
    app: &mut App,
    admin: &Addr,
    reward_token: Addr,
    reward_addr: &Addr,
    amount: u128,
) {
    let fund_sub_msg = to_json_binary(&ReceiveMsg::Fund {}).unwrap();
    let fund_msg = Cw20ExecuteMsg::Send {
        contract: reward_addr.clone().into_string(),
        amount: Uint128::new(amount),
        msg: fund_sub_msg,
    };
    let _res = app
        .borrow_mut()
        .execute_contract(admin.clone(), reward_token, &fund_msg, &[])
        .unwrap();
}

#[test]
fn test_zero_rewards_duration() {
    let mut app = mock_app();
    let admin = Addr::unchecked(OWNER);
    app.borrow_mut().update_block(|b| b.height = 0);
    let denom = "utest".to_string();
    let (staking_addr, _) = setup_staking_contract(&mut app, vec![]);
    let reward_funding = vec![coin(100000000, denom.clone())];
    app.sudo(SudoMsg::Bank({
        BankSudo::Mint {
            to_address: admin.to_string(),
            amount: reward_funding,
        }
    }))
    .unwrap();

    let reward_token = Denom::Native(denom);
    let owner = admin;
    let reward_code_id = app.store_code(cw20_stake_external_rewards_contract());
    let msg = crate::msg::InstantiateMsg {
        owner: Some(owner.clone().into_string()),
        staking_contract: staking_addr.to_string(),
        reward_token,
        reward_duration: 0,
    };
    let err: ContractError = app
        .instantiate_contract(reward_code_id, owner, &msg, &[], "reward", None)
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::ZeroRewardDuration {})
}

#[test]
fn test_native_rewards() {
    let mut app = mock_app();
    let admin = Addr::unchecked(OWNER);
    app.borrow_mut().update_block(|b| b.height = 0);
    let initial_balances = vec![
        Cw20Coin {
            address: ADDR1.to_string(),
            amount: Uint128::new(100),
        },
        Cw20Coin {
            address: ADDR2.to_string(),
            amount: Uint128::new(50),
        },
        Cw20Coin {
            address: ADDR3.to_string(),
            amount: Uint128::new(50),
        },
    ];
    let denom = "utest".to_string();
    let (staking_addr, cw20_addr) = setup_staking_contract(&mut app, initial_balances);
    let reward_funding = vec![coin(100000000, denom.clone())];
    app.sudo(SudoMsg::Bank({
        BankSudo::Mint {
            to_address: admin.to_string(),
            amount: reward_funding.clone(),
        }
    }))
    .unwrap();
    let reward_addr = setup_reward_contract(
        &mut app,
        staking_addr.clone(),
        Denom::Native(denom.clone()),
        admin.clone(),
    );

    app.borrow_mut().update_block(|b| b.height = 1000);

    let fund_msg = ExecuteMsg::Fund {};

    let _res = app
        .borrow_mut()
        .execute_contract(
            admin.clone(),
            reward_addr.clone(),
            &fund_msg,
            &reward_funding,
        )
        .unwrap();

    let res: InfoResponse = app
        .borrow_mut()
        .wrap()
        .query_wasm_smart(&reward_addr, &QueryMsg::Info {})
        .unwrap();

    assert_eq!(res.reward.reward_rate, Uint128::new(1000));
    assert_eq!(res.reward.period_finish, 101000);
    assert_eq!(res.reward.reward_duration, 100000);

    app.borrow_mut().update_block(next_block);
    assert_pending_rewards(&mut app, &reward_addr, ADDR1, 500);
    assert_pending_rewards(&mut app, &reward_addr, ADDR2, 250);
    assert_pending_rewards(&mut app, &reward_addr, ADDR3, 250);

    app.borrow_mut().update_block(next_block);
    assert_pending_rewards(&mut app, &reward_addr, ADDR1, 1000);
    assert_pending_rewards(&mut app, &reward_addr, ADDR2, 500);
    assert_pending_rewards(&mut app, &reward_addr, ADDR3, 500);

    app.borrow_mut().update_block(next_block);
    assert_pending_rewards(&mut app, &reward_addr, ADDR1, 1500);
    assert_pending_rewards(&mut app, &reward_addr, ADDR2, 750);
    assert_pending_rewards(&mut app, &reward_addr, ADDR3, 750);

    app.borrow_mut().update_block(next_block);
    assert_pending_rewards(&mut app, &reward_addr, ADDR1, 2000);
    assert_pending_rewards(&mut app, &reward_addr, ADDR2, 1000);
    assert_pending_rewards(&mut app, &reward_addr, ADDR3, 1000);

    assert_eq!(get_balance_native(&app, ADDR1, &denom), Uint128::zero());
    claim_rewards(&mut app, reward_addr.clone(), ADDR1);
    assert_eq!(get_balance_native(&app, ADDR1, &denom), Uint128::new(2000));
    assert_pending_rewards(&mut app, &reward_addr, ADDR1, 0);

    app.borrow_mut().update_block(|b| b.height += 10);
    assert_pending_rewards(&mut app, &reward_addr, ADDR1, 5000);
    assert_pending_rewards(&mut app, &reward_addr, ADDR2, 3500);
    assert_pending_rewards(&mut app, &reward_addr, ADDR3, 3500);

    unstake_tokens(&mut app, &staking_addr, ADDR2, 50);
    unstake_tokens(&mut app, &staking_addr, ADDR3, 50);

    app.borrow_mut().update_block(|b| b.height += 10);
    assert_pending_rewards(&mut app, &reward_addr, ADDR1, 15000);
    assert_pending_rewards(&mut app, &reward_addr, ADDR2, 3500);
    assert_pending_rewards(&mut app, &reward_addr, ADDR3, 3500);

    claim_rewards(&mut app, reward_addr.clone(), ADDR1);
    assert_eq!(get_balance_native(&app, ADDR1, &denom), Uint128::new(17000));

    claim_rewards(&mut app, reward_addr.clone(), ADDR2);
    assert_eq!(get_balance_native(&app, ADDR2, &denom), Uint128::new(3500));

    stake_tokens(&mut app, &staking_addr, &cw20_addr, ADDR2, 50);
    stake_tokens(&mut app, &staking_addr, &cw20_addr, ADDR3, 50);

    app.borrow_mut().update_block(|b| b.height += 10);
    assert_pending_rewards(&mut app, &reward_addr, ADDR1, 5000);
    assert_pending_rewards(&mut app, &reward_addr, ADDR2, 2500);
    assert_pending_rewards(&mut app, &reward_addr, ADDR3, 6000);

    // Current height is 1034. ADDR1 is receiving 500 tokens/block
    // and ADDR2 / ADDR3 are receiving 250.
    //
    // At height 101000 99966 additional blocks have passed. So we
    // expect:
    //
    // ADDR1: 5000 + 99966 * 500 = 49,998,000
    // ADDR2: 2500 + 99966 * 250 = 24,994,000
    // ADDR3: 6000 + 99966 * 250 = 24,997,500
    app.borrow_mut().update_block(|b| b.height = 101000);
    assert_pending_rewards(&mut app, &reward_addr, ADDR1, 49988000);
    assert_pending_rewards(&mut app, &reward_addr, ADDR2, 24994000);
    assert_pending_rewards(&mut app, &reward_addr, ADDR3, 24997500);

    claim_rewards(&mut app, reward_addr.clone(), ADDR1);
    claim_rewards(&mut app, reward_addr.clone(), ADDR2);
    assert_eq!(
        get_balance_native(&app, ADDR1, &denom),
        Uint128::new(50005000)
    );
    assert_eq!(
        get_balance_native(&app, ADDR2, &denom),
        Uint128::new(24997500)
    );
    assert_eq!(get_balance_native(&app, ADDR3, &denom), Uint128::new(0));
    assert_eq!(
        get_balance_native(&app, &reward_addr, &denom),
        Uint128::new(24997500)
    );

    app.borrow_mut().update_block(|b| b.height = 200000);
    let fund_msg = ExecuteMsg::Fund {};

    // Add more rewards
    let reward_funding = vec![coin(200000000, denom.clone())];
    app.sudo(SudoMsg::Bank({
        BankSudo::Mint {
            to_address: admin.to_string(),
            amount: reward_funding.clone(),
        }
    }))
    .unwrap();

    let _res = app
        .borrow_mut()
        .execute_contract(
            admin.clone(),
            reward_addr.clone(),
            &fund_msg,
            &reward_funding,
        )
        .unwrap();

    app.borrow_mut().update_block(|b| b.height = 300000);
    assert_pending_rewards(&mut app, &reward_addr, ADDR1, 100000000);
    assert_pending_rewards(&mut app, &reward_addr, ADDR2, 50000000);
    assert_pending_rewards(&mut app, &reward_addr, ADDR3, 74997500);

    claim_rewards(&mut app, reward_addr.clone(), ADDR1);
    claim_rewards(&mut app, reward_addr.clone(), ADDR2);
    assert_eq!(
        get_balance_native(&app, ADDR1, &denom),
        Uint128::new(150005000)
    );
    assert_eq!(
        get_balance_native(&app, ADDR2, &denom),
        Uint128::new(74997500)
    );
    assert_eq!(get_balance_native(&app, ADDR3, &denom), Uint128::zero());
    assert_eq!(
        get_balance_native(&app, &reward_addr, &denom),
        Uint128::new(74997500)
    );

    // Add more rewards
    let reward_funding = vec![coin(200000000, denom.clone())];
    app.sudo(SudoMsg::Bank({
        BankSudo::Mint {
            to_address: admin.to_string(),
            amount: reward_funding.clone(),
        }
    }))
    .unwrap();

    let _res = app
        .borrow_mut()
        .execute_contract(admin, reward_addr.clone(), &fund_msg, &reward_funding)
        .unwrap();

    app.borrow_mut().update_block(|b| b.height = 400000);
    assert_pending_rewards(&mut app, &reward_addr, ADDR1, 100000000);
    assert_pending_rewards(&mut app, &reward_addr, ADDR2, 50000000);
    assert_pending_rewards(&mut app, &reward_addr, ADDR3, 124997500);

    claim_rewards(&mut app, reward_addr.clone(), ADDR1);
    claim_rewards(&mut app, reward_addr.clone(), ADDR2);
    claim_rewards(&mut app, reward_addr.clone(), ADDR3);
    assert_eq!(
        get_balance_native(&app, ADDR1, &denom),
        Uint128::new(250005000)
    );
    assert_eq!(
        get_balance_native(&app, ADDR2, &denom),
        Uint128::new(124997500)
    );
    assert_eq!(
        get_balance_native(&app, ADDR3, &denom),
        Uint128::new(124997500)
    );
    assert_eq!(
        get_balance_native(&app, &reward_addr, &denom),
        Uint128::zero()
    );

    app.borrow_mut().update_block(|b| b.height = 500000);
    assert_pending_rewards(&mut app, &reward_addr, ADDR1, 0);
    assert_pending_rewards(&mut app, &reward_addr, ADDR2, 0);
    assert_pending_rewards(&mut app, &reward_addr, ADDR3, 0);

    app.borrow_mut().update_block(|b| b.height = 1000000);
    unstake_tokens(&mut app, &staking_addr, ADDR3, 1);
    stake_tokens(&mut app, &staking_addr, &cw20_addr, ADDR3, 1);
}

#[test]
fn test_cw20_rewards() {
    let mut app = mock_app();
    let admin = Addr::unchecked(OWNER);
    app.borrow_mut().update_block(|b| b.height = 0);
    let initial_balances = vec![
        Cw20Coin {
            address: ADDR1.to_string(),
            amount: Uint128::new(100),
        },
        Cw20Coin {
            address: ADDR2.to_string(),
            amount: Uint128::new(50),
        },
        Cw20Coin {
            address: ADDR3.to_string(),
            amount: Uint128::new(50),
        },
    ];
    let denom = "utest".to_string();
    let (staking_addr, cw20_addr) = setup_staking_contract(&mut app, initial_balances);
    let reward_token = instantiate_cw20(
        &mut app,
        vec![Cw20Coin {
            address: OWNER.to_string(),
            amount: Uint128::new(500000000),
        }],
    );
    let reward_addr = setup_reward_contract(
        &mut app,
        staking_addr.clone(),
        Denom::Cw20(reward_token.clone()),
        admin.clone(),
    );

    app.borrow_mut().update_block(|b| b.height = 1000);

    fund_rewards_cw20(
        &mut app,
        &admin,
        reward_token.clone(),
        &reward_addr,
        100000000,
    );

    let res: InfoResponse = app
        .borrow_mut()
        .wrap()
        .query_wasm_smart(&reward_addr, &QueryMsg::Info {})
        .unwrap();

    assert_eq!(res.reward.reward_rate, Uint128::new(1000));
    assert_eq!(res.reward.period_finish, 101000);
    assert_eq!(res.reward.reward_duration, 100000);

    app.borrow_mut().update_block(next_block);
    assert_pending_rewards(&mut app, &reward_addr, ADDR1, 500);
    assert_pending_rewards(&mut app, &reward_addr, ADDR2, 250);
    assert_pending_rewards(&mut app, &reward_addr, ADDR3, 250);

    app.borrow_mut().update_block(next_block);
    assert_pending_rewards(&mut app, &reward_addr, ADDR1, 1000);
    assert_pending_rewards(&mut app, &reward_addr, ADDR2, 500);
    assert_pending_rewards(&mut app, &reward_addr, ADDR3, 500);

    app.borrow_mut().update_block(next_block);
    assert_pending_rewards(&mut app, &reward_addr, ADDR1, 1500);
    assert_pending_rewards(&mut app, &reward_addr, ADDR2, 750);
    assert_pending_rewards(&mut app, &reward_addr, ADDR3, 750);

    app.borrow_mut().update_block(next_block);
    assert_pending_rewards(&mut app, &reward_addr, ADDR1, 2000);
    assert_pending_rewards(&mut app, &reward_addr, ADDR2, 1000);
    assert_pending_rewards(&mut app, &reward_addr, ADDR3, 1000);

    assert_eq!(
        get_balance_cw20(&app, &reward_token, ADDR1),
        Uint128::zero()
    );
    claim_rewards(&mut app, reward_addr.clone(), ADDR1);
    assert_eq!(
        get_balance_cw20(&app, &reward_token, ADDR1),
        Uint128::new(2000)
    );
    assert_pending_rewards(&mut app, &reward_addr, ADDR1, 0);

    app.borrow_mut().update_block(|b| b.height += 10);
    assert_pending_rewards(&mut app, &reward_addr, ADDR1, 5000);
    assert_pending_rewards(&mut app, &reward_addr, ADDR2, 3500);
    assert_pending_rewards(&mut app, &reward_addr, ADDR3, 3500);

    unstake_tokens(&mut app, &staking_addr, ADDR2, 50);
    unstake_tokens(&mut app, &staking_addr, ADDR3, 50);

    app.borrow_mut().update_block(|b| b.height += 10);
    assert_pending_rewards(&mut app, &reward_addr, ADDR1, 15000);
    assert_pending_rewards(&mut app, &reward_addr, ADDR2, 3500);
    assert_pending_rewards(&mut app, &reward_addr, ADDR3, 3500);

    claim_rewards(&mut app, reward_addr.clone(), ADDR1);
    assert_eq!(
        get_balance_cw20(&app, &reward_token, ADDR1),
        Uint128::new(17000)
    );

    claim_rewards(&mut app, reward_addr.clone(), ADDR2);
    assert_eq!(
        get_balance_cw20(&app, &reward_token, ADDR2),
        Uint128::new(3500)
    );

    stake_tokens(&mut app, &staking_addr, &cw20_addr, ADDR2, 50);
    stake_tokens(&mut app, &staking_addr, &cw20_addr, ADDR3, 50);

    app.borrow_mut().update_block(|b| b.height += 10);
    assert_pending_rewards(&mut app, &reward_addr, ADDR1, 5000);
    assert_pending_rewards(&mut app, &reward_addr, ADDR2, 2500);
    assert_pending_rewards(&mut app, &reward_addr, ADDR3, 6000);

    app.borrow_mut().update_block(|b| b.height = 101000);
    assert_pending_rewards(&mut app, &reward_addr, ADDR1, 49988000);
    assert_pending_rewards(&mut app, &reward_addr, ADDR2, 24994000);
    assert_pending_rewards(&mut app, &reward_addr, ADDR3, 24997500);

    claim_rewards(&mut app, reward_addr.clone(), ADDR1);
    claim_rewards(&mut app, reward_addr.clone(), ADDR2);
    assert_eq!(
        get_balance_cw20(&app, &reward_token, ADDR1),
        Uint128::new(50005000)
    );
    assert_eq!(
        get_balance_cw20(&app, &reward_token, ADDR2),
        Uint128::new(24997500)
    );
    assert_eq!(
        get_balance_cw20(&app, &reward_token, ADDR3),
        Uint128::new(0)
    );
    assert_eq!(
        get_balance_cw20(&app, &reward_token, &reward_addr),
        Uint128::new(24997500)
    );

    app.borrow_mut().update_block(|b| b.height = 200000);

    let reward_funding = vec![coin(200000000, denom)];
    app.sudo(SudoMsg::Bank({
        BankSudo::Mint {
            to_address: admin.to_string(),
            amount: reward_funding,
        }
    }))
    .unwrap();

    fund_rewards_cw20(
        &mut app,
        &admin,
        reward_token.clone(),
        &reward_addr,
        200000000,
    );

    app.borrow_mut().update_block(|b| b.height = 300000);
    assert_pending_rewards(&mut app, &reward_addr, ADDR1, 100000000);
    assert_pending_rewards(&mut app, &reward_addr, ADDR2, 50000000);
    assert_pending_rewards(&mut app, &reward_addr, ADDR3, 74997500);

    claim_rewards(&mut app, reward_addr.clone(), ADDR1);
    claim_rewards(&mut app, reward_addr.clone(), ADDR2);
    assert_eq!(
        get_balance_cw20(&app, &reward_token, ADDR1),
        Uint128::new(150005000)
    );
    assert_eq!(
        get_balance_cw20(&app, &reward_token, ADDR2),
        Uint128::new(74997500)
    );
    assert_eq!(
        get_balance_cw20(&app, &reward_token, ADDR3),
        Uint128::zero()
    );
    assert_eq!(
        get_balance_cw20(&app, &reward_token, &reward_addr),
        Uint128::new(74997500)
    );

    // Add more rewards
    fund_rewards_cw20(
        &mut app,
        &admin,
        reward_token.clone(),
        &reward_addr,
        200000000,
    );

    app.borrow_mut().update_block(|b| b.height = 400000);
    assert_pending_rewards(&mut app, &reward_addr, ADDR1, 100000000);
    assert_pending_rewards(&mut app, &reward_addr, ADDR2, 50000000);
    assert_pending_rewards(&mut app, &reward_addr, ADDR3, 124997500);

    claim_rewards(&mut app, reward_addr.clone(), ADDR1);
    claim_rewards(&mut app, reward_addr.clone(), ADDR2);
    claim_rewards(&mut app, reward_addr.clone(), ADDR3);
    assert_eq!(
        get_balance_cw20(&app, &reward_token, ADDR1),
        Uint128::new(250005000)
    );
    assert_eq!(
        get_balance_cw20(&app, &reward_token, ADDR2),
        Uint128::new(124997500)
    );
    assert_eq!(
        get_balance_cw20(&app, &reward_token, ADDR3),
        Uint128::new(124997500)
    );
    assert_eq!(
        get_balance_cw20(&app, &reward_token, &reward_addr),
        Uint128::zero()
    );

    app.borrow_mut().update_block(|b| b.height = 500000);
    assert_pending_rewards(&mut app, &reward_addr, ADDR1, 0);
    assert_pending_rewards(&mut app, &reward_addr, ADDR2, 0);
    assert_pending_rewards(&mut app, &reward_addr, ADDR3, 0);

    app.borrow_mut().update_block(|b| b.height = 1000000);
    unstake_tokens(&mut app, &staking_addr, ADDR3, 1);
    stake_tokens(&mut app, &staking_addr, &cw20_addr, ADDR3, 1);
}

#[test]
fn update_rewards() {
    let mut app = mock_app();
    let admin = Addr::unchecked(OWNER);
    app.borrow_mut().update_block(|b| b.height = 0);
    let initial_balances = vec![
        Cw20Coin {
            address: ADDR1.to_string(),
            amount: Uint128::new(100),
        },
        Cw20Coin {
            address: ADDR2.to_string(),
            amount: Uint128::new(50),
        },
        Cw20Coin {
            address: ADDR3.to_string(),
            amount: Uint128::new(50),
        },
    ];
    let denom = "utest".to_string();
    let (staking_addr, _cw20_addr) = setup_staking_contract(&mut app, initial_balances);
    let reward_funding = vec![coin(200000000, denom.clone())];
    app.sudo(SudoMsg::Bank({
        BankSudo::Mint {
            to_address: admin.to_string(),
            amount: reward_funding.clone(),
        }
    }))
    .unwrap();
    // Add funding to Addr1 to make sure it can't update staking contract
    app.sudo(SudoMsg::Bank({
        BankSudo::Mint {
            to_address: ADDR1.to_string(),
            amount: reward_funding.clone(),
        }
    }))
    .unwrap();
    let reward_addr = setup_reward_contract(
        &mut app,
        staking_addr,
        Denom::Native(denom.clone()),
        admin.clone(),
    );

    app.borrow_mut().update_block(|b| b.height = 1000);

    let fund_msg = ExecuteMsg::Fund {};

    // None admin cannot update rewards
    let err: ContractError = app
        .borrow_mut()
        .execute_contract(
            Addr::unchecked(ADDR1),
            reward_addr.clone(),
            &fund_msg,
            &reward_funding,
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert_eq!(err, ContractError::Ownable(OwnershipError::NotOwner));

    let _res = app
        .borrow_mut()
        .execute_contract(
            admin.clone(),
            reward_addr.clone(),
            &fund_msg,
            &reward_funding,
        )
        .unwrap();

    let res: InfoResponse = app
        .borrow_mut()
        .wrap()
        .query_wasm_smart(&reward_addr, &QueryMsg::Info {})
        .unwrap();

    assert_eq!(res.reward.reward_rate, Uint128::new(2000));
    assert_eq!(res.reward.period_finish, 101000);
    assert_eq!(res.reward.reward_duration, 100000);

    // Create new period after old period
    app.borrow_mut().update_block(|b| b.height = 101000);

    let reward_funding = vec![coin(100000000, denom.clone())];
    app.sudo(SudoMsg::Bank({
        BankSudo::Mint {
            to_address: admin.to_string(),
            amount: reward_funding.clone(),
        }
    }))
    .unwrap();
    let _res = app
        .borrow_mut()
        .execute_contract(
            admin.clone(),
            reward_addr.clone(),
            &fund_msg,
            &reward_funding,
        )
        .unwrap();

    let res: InfoResponse = app
        .borrow_mut()
        .wrap()
        .query_wasm_smart(&reward_addr, &QueryMsg::Info {})
        .unwrap();

    assert_eq!(res.reward.reward_rate, Uint128::new(1000));
    assert_eq!(res.reward.period_finish, 201000);
    assert_eq!(res.reward.reward_duration, 100000);

    // Add funds in middle of period returns an error
    app.borrow_mut().update_block(|b| b.height = 151000);

    let reward_funding = vec![coin(200000000, denom)];
    app.sudo(SudoMsg::Bank({
        BankSudo::Mint {
            to_address: admin.to_string(),
            amount: reward_funding.clone(),
        }
    }))
    .unwrap();
    let err = app
        .borrow_mut()
        .execute_contract(admin, reward_addr.clone(), &fund_msg, &reward_funding)
        .unwrap_err();
    assert_eq!(
        ContractError::RewardPeriodNotFinished {},
        err.downcast().unwrap()
    );

    let res: InfoResponse = app
        .borrow_mut()
        .wrap()
        .query_wasm_smart(&reward_addr, &QueryMsg::Info {})
        .unwrap();

    assert_eq!(res.reward.reward_rate, Uint128::new(1000));
    assert_eq!(res.reward.period_finish, 201000);
    assert_eq!(res.reward.reward_duration, 100000);
}

#[test]
fn update_reward_duration() {
    let mut app = mock_app();
    let admin = Addr::unchecked(OWNER);
    app.borrow_mut().update_block(|b| b.height = 0);
    let initial_balances = vec![
        Cw20Coin {
            address: ADDR1.to_string(),
            amount: Uint128::new(100),
        },
        Cw20Coin {
            address: ADDR2.to_string(),
            amount: Uint128::new(50),
        },
        Cw20Coin {
            address: ADDR3.to_string(),
            amount: Uint128::new(50),
        },
    ];
    let denom = "utest".to_string();
    let (staking_addr, _cw20_addr) = setup_staking_contract(&mut app, initial_balances);

    let reward_addr = setup_reward_contract(
        &mut app,
        staking_addr,
        Denom::Native(denom.clone()),
        admin.clone(),
    );

    let res: InfoResponse = app
        .borrow_mut()
        .wrap()
        .query_wasm_smart(&reward_addr, &QueryMsg::Info {})
        .unwrap();

    assert_eq!(res.reward.reward_rate, Uint128::new(0));
    assert_eq!(res.reward.period_finish, 0);
    assert_eq!(res.reward.reward_duration, 100000);

    // Zero rewards durations are not allowed.
    let msg = ExecuteMsg::UpdateRewardDuration { new_duration: 0 };
    let err: ContractError = app
        .borrow_mut()
        .execute_contract(admin.clone(), reward_addr.clone(), &msg, &[])
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::ZeroRewardDuration {});

    let msg = ExecuteMsg::UpdateRewardDuration { new_duration: 10 };
    let _resp = app
        .borrow_mut()
        .execute_contract(admin.clone(), reward_addr.clone(), &msg, &[])
        .unwrap();

    let res: InfoResponse = app
        .borrow_mut()
        .wrap()
        .query_wasm_smart(&reward_addr, &QueryMsg::Info {})
        .unwrap();

    assert_eq!(res.reward.reward_rate, Uint128::new(0));
    assert_eq!(res.reward.period_finish, 0);
    assert_eq!(res.reward.reward_duration, 10);

    // Non-admin cannot update rewards
    let msg = ExecuteMsg::UpdateRewardDuration { new_duration: 100 };
    let err: ContractError = app
        .borrow_mut()
        .execute_contract(Addr::unchecked("non-admin"), reward_addr.clone(), &msg, &[])
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::Ownable(OwnershipError::NotOwner));

    let reward_funding = vec![coin(1000, denom)];
    app.sudo(SudoMsg::Bank({
        BankSudo::Mint {
            to_address: admin.to_string(),
            amount: reward_funding.clone(),
        }
    }))
    .unwrap();
    // Add funding to Addr1 to make sure it can't update staking contract
    app.sudo(SudoMsg::Bank({
        BankSudo::Mint {
            to_address: ADDR1.to_string(),
            amount: reward_funding.clone(),
        }
    }))
    .unwrap();

    app.borrow_mut().update_block(|b| b.height = 1000);

    let fund_msg = ExecuteMsg::Fund {};

    let _res = app
        .borrow_mut()
        .execute_contract(
            admin.clone(),
            reward_addr.clone(),
            &fund_msg,
            &reward_funding,
        )
        .unwrap();

    let res: InfoResponse = app
        .borrow_mut()
        .wrap()
        .query_wasm_smart(&reward_addr, &QueryMsg::Info {})
        .unwrap();

    assert_eq!(res.reward.reward_rate, Uint128::new(100));
    assert_eq!(res.reward.period_finish, 1010);
    assert_eq!(res.reward.reward_duration, 10);

    // Cannot update reward period before it finishes
    let msg = ExecuteMsg::UpdateRewardDuration { new_duration: 10 };
    let err: ContractError = app
        .borrow_mut()
        .execute_contract(admin.clone(), reward_addr.clone(), &msg, &[])
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::RewardPeriodNotFinished {});

    // Update reward period once rewards are finished
    app.borrow_mut().update_block(|b| b.height = 1010);

    let msg = ExecuteMsg::UpdateRewardDuration { new_duration: 100 };
    let _resp = app
        .borrow_mut()
        .execute_contract(admin, reward_addr.clone(), &msg, &[])
        .unwrap();

    let res: InfoResponse = app
        .borrow_mut()
        .wrap()
        .query_wasm_smart(&reward_addr, &QueryMsg::Info {})
        .unwrap();

    assert_eq!(res.reward.reward_rate, Uint128::new(100));
    assert_eq!(res.reward.period_finish, 1010);
    assert_eq!(res.reward.reward_duration, 100);
}

#[test]
fn test_update_owner() {
    let mut app = mock_app();
    let addr_owner = Addr::unchecked(OWNER);
    app.borrow_mut().update_block(|b| b.height = 0);
    let initial_balances = vec![
        Cw20Coin {
            address: ADDR1.to_string(),
            amount: Uint128::new(100),
        },
        Cw20Coin {
            address: ADDR2.to_string(),
            amount: Uint128::new(50),
        },
        Cw20Coin {
            address: ADDR3.to_string(),
            amount: Uint128::new(50),
        },
    ];
    let denom = "utest".to_string();
    let (staking_addr, _cw20_addr) = setup_staking_contract(&mut app, initial_balances);

    let reward_addr = setup_reward_contract(
        &mut app,
        staking_addr,
        Denom::Native(denom),
        addr_owner.clone(),
    );

    let owner = get_ownership(&app, &reward_addr).owner;
    assert_eq!(owner, Some(addr_owner.clone()));

    // random addr cannot update owner
    let msg = ExecuteMsg::UpdateOwnership(Action::TransferOwnership {
        new_owner: ADDR1.to_string(),
        expiry: None,
    });
    let err: ContractError = app
        .borrow_mut()
        .execute_contract(Addr::unchecked(ADDR1), reward_addr.clone(), &msg, &[])
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::Ownable(OwnershipError::NotOwner));

    // owner nominates a new onwer.
    app.borrow_mut()
        .execute_contract(addr_owner.clone(), reward_addr.clone(), &msg, &[])
        .unwrap();

    let ownership = get_ownership(&app, &reward_addr);
    assert_eq!(
        ownership,
        Ownership::<Addr> {
            owner: Some(addr_owner),
            pending_owner: Some(Addr::unchecked(ADDR1)),
            pending_expiry: None,
        }
    );

    // new owner accepts the nomination.
    app.execute_contract(
        Addr::unchecked(ADDR1),
        reward_addr.clone(),
        &ExecuteMsg::UpdateOwnership(Action::AcceptOwnership),
        &[],
    )
    .unwrap();

    let ownership = get_ownership(&app, &reward_addr);
    assert_eq!(
        ownership,
        Ownership::<Addr> {
            owner: Some(Addr::unchecked(ADDR1)),
            pending_owner: None,
            pending_expiry: None,
        }
    );

    // new owner renounces ownership.
    app.execute_contract(
        Addr::unchecked(ADDR1),
        reward_addr.clone(),
        &ExecuteMsg::UpdateOwnership(Action::RenounceOwnership),
        &[],
    )
    .unwrap();

    let ownership = get_ownership(&app, &reward_addr);
    assert_eq!(
        ownership,
        Ownership::<Addr> {
            owner: None,
            pending_owner: None,
            pending_expiry: None,
        }
    );
}

#[test]
fn test_cannot_fund_with_wrong_coin_native() {
    let mut app = mock_app();
    let owner = Addr::unchecked(OWNER);
    app.borrow_mut().update_block(|b| b.height = 0);
    let initial_balances = vec![
        Cw20Coin {
            address: ADDR1.to_string(),
            amount: Uint128::new(100),
        },
        Cw20Coin {
            address: ADDR2.to_string(),
            amount: Uint128::new(50),
        },
        Cw20Coin {
            address: ADDR3.to_string(),
            amount: Uint128::new(50),
        },
    ];
    let denom = "utest".to_string();
    let (staking_addr, _cw20_addr) = setup_staking_contract(&mut app, initial_balances);

    let reward_addr = setup_reward_contract(
        &mut app,
        staking_addr,
        Denom::Native(denom.clone()),
        owner.clone(),
    );

    app.borrow_mut().update_block(|b| b.height = 1000);

    // No funding
    let fund_msg = ExecuteMsg::Fund {};

    let err: ContractError = app
        .borrow_mut()
        .execute_contract(owner.clone(), reward_addr.clone(), &fund_msg, &[])
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::InvalidFunds {});

    // Invalid funding
    let invalid_funding = vec![coin(100, "invalid")];
    app.sudo(SudoMsg::Bank({
        BankSudo::Mint {
            to_address: owner.to_string(),
            amount: invalid_funding.clone(),
        }
    }))
    .unwrap();

    let fund_msg = ExecuteMsg::Fund {};

    let err: ContractError = app
        .borrow_mut()
        .execute_contract(
            owner.clone(),
            reward_addr.clone(),
            &fund_msg,
            &invalid_funding,
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::InvalidFunds {});

    // Extra funding
    let extra_funding = vec![coin(100, denom), coin(100, "extra")];
    app.sudo(SudoMsg::Bank({
        BankSudo::Mint {
            to_address: owner.to_string(),
            amount: extra_funding.clone(),
        }
    }))
    .unwrap();

    let fund_msg = ExecuteMsg::Fund {};

    let err: ContractError = app
        .borrow_mut()
        .execute_contract(
            owner.clone(),
            reward_addr.clone(),
            &fund_msg,
            &extra_funding,
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::InvalidFunds {});

    // Cw20 funding fails
    let cw20_token = instantiate_cw20(
        &mut app,
        vec![Cw20Coin {
            address: OWNER.to_string(),
            amount: Uint128::new(500000000),
        }],
    );
    let fund_sub_msg = to_json_binary(&ReceiveMsg::Fund {}).unwrap();
    let fund_msg = Cw20ExecuteMsg::Send {
        contract: reward_addr.into_string(),
        amount: Uint128::new(100),
        msg: fund_sub_msg,
    };
    let err: ContractError = app
        .borrow_mut()
        .execute_contract(owner, cw20_token, &fund_msg, &[])
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::InvalidCw20 {});
}

#[test]
fn test_cannot_fund_with_wrong_coin_cw20() {
    let mut app = mock_app();
    let admin = Addr::unchecked(OWNER);
    app.borrow_mut().update_block(|b| b.height = 0);
    let initial_balances = vec![
        Cw20Coin {
            address: ADDR1.to_string(),
            amount: Uint128::new(100),
        },
        Cw20Coin {
            address: ADDR2.to_string(),
            amount: Uint128::new(50),
        },
        Cw20Coin {
            address: ADDR3.to_string(),
            amount: Uint128::new(50),
        },
    ];
    let _denom = "utest".to_string();
    let (staking_addr, _cw20_addr) = setup_staking_contract(&mut app, initial_balances);
    let reward_token = instantiate_cw20(
        &mut app,
        vec![Cw20Coin {
            address: OWNER.to_string(),
            amount: Uint128::new(500000000),
        }],
    );
    let reward_addr = setup_reward_contract(
        &mut app,
        staking_addr,
        Denom::Cw20(Addr::unchecked("dummy_cw20")),
        admin.clone(),
    );

    app.borrow_mut().update_block(|b| b.height = 1000);

    // Test with invalid token
    let fund_sub_msg = to_json_binary(&ReceiveMsg::Fund {}).unwrap();
    let fund_msg = Cw20ExecuteMsg::Send {
        contract: reward_addr.clone().into_string(),
        amount: Uint128::new(100),
        msg: fund_sub_msg,
    };
    let err: ContractError = app
        .borrow_mut()
        .execute_contract(admin.clone(), reward_token, &fund_msg, &[])
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::InvalidCw20 {});

    // Test does not work when funded with native
    let invalid_funding = vec![coin(100, "invalid")];
    app.sudo(SudoMsg::Bank({
        BankSudo::Mint {
            to_address: admin.to_string(),
            amount: invalid_funding.clone(),
        }
    }))
    .unwrap();

    let fund_msg = ExecuteMsg::Fund {};

    let err: ContractError = app
        .borrow_mut()
        .execute_contract(admin, reward_addr, &fund_msg, &invalid_funding)
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::InvalidFunds {})
}

#[test]
fn test_rewards_with_zero_staked() {
    let mut app = mock_app();
    let admin = Addr::unchecked(OWNER);
    app.borrow_mut().update_block(|b| b.height = 0);
    let initial_balances = vec![
        Cw20Coin {
            address: ADDR1.to_string(),
            amount: Uint128::new(100),
        },
        Cw20Coin {
            address: ADDR2.to_string(),
            amount: Uint128::new(50),
        },
        Cw20Coin {
            address: ADDR3.to_string(),
            amount: Uint128::new(50),
        },
    ];
    let denom = "utest".to_string();
    // Instantiate cw20 contract
    let cw20_addr = instantiate_cw20(&mut app, initial_balances.clone());
    app.update_block(next_block);
    // Instantiate staking contract
    let staking_addr = instantiate_staking(&mut app, cw20_addr.clone(), None);
    app.update_block(next_block);
    let reward_funding = vec![coin(100000000, denom.clone())];
    app.sudo(SudoMsg::Bank({
        BankSudo::Mint {
            to_address: admin.to_string(),
            amount: reward_funding.clone(),
        }
    }))
    .unwrap();
    let reward_addr = setup_reward_contract(
        &mut app,
        staking_addr.clone(),
        Denom::Native(denom),
        admin.clone(),
    );

    app.borrow_mut().update_block(|b| b.height = 1000);

    let fund_msg = ExecuteMsg::Fund {};

    let _res = app
        .borrow_mut()
        .execute_contract(admin, reward_addr.clone(), &fund_msg, &reward_funding)
        .unwrap();

    let res: InfoResponse = app
        .borrow_mut()
        .wrap()
        .query_wasm_smart(&reward_addr, &QueryMsg::Info {})
        .unwrap();

    assert_eq!(res.reward.reward_rate, Uint128::new(1000));
    assert_eq!(res.reward.period_finish, 101000);
    assert_eq!(res.reward.reward_duration, 100000);

    app.borrow_mut().update_block(next_block);
    assert_pending_rewards(&mut app, &reward_addr, ADDR1, 0);
    assert_pending_rewards(&mut app, &reward_addr, ADDR2, 0);
    assert_pending_rewards(&mut app, &reward_addr, ADDR3, 0);

    for coin in initial_balances {
        stake_tokens(
            &mut app,
            &staking_addr,
            &cw20_addr,
            coin.address,
            coin.amount.u128(),
        );
    }

    app.borrow_mut().update_block(next_block);
    assert_pending_rewards(&mut app, &reward_addr, ADDR1, 500);
    assert_pending_rewards(&mut app, &reward_addr, ADDR2, 250);
    assert_pending_rewards(&mut app, &reward_addr, ADDR3, 250);

    app.borrow_mut().update_block(next_block);
    assert_pending_rewards(&mut app, &reward_addr, ADDR1, 1000);
    assert_pending_rewards(&mut app, &reward_addr, ADDR2, 500);
    assert_pending_rewards(&mut app, &reward_addr, ADDR3, 500);
}

#[test]
fn test_small_rewards() {
    // This test was added due to a bug in the contract not properly paying out small reward
    // amounts due to floor division
    let mut app = mock_app();
    let admin = Addr::unchecked(OWNER);
    app.borrow_mut().update_block(|b| b.height = 0);
    let initial_balances = vec![
        Cw20Coin {
            address: ADDR1.to_string(),
            amount: Uint128::new(100),
        },
        Cw20Coin {
            address: ADDR2.to_string(),
            amount: Uint128::new(50),
        },
        Cw20Coin {
            address: ADDR3.to_string(),
            amount: Uint128::new(50),
        },
    ];
    let denom = "utest".to_string();
    let (staking_addr, _) = setup_staking_contract(&mut app, initial_balances);
    let reward_funding = vec![coin(1000000, denom.clone())];
    app.sudo(SudoMsg::Bank({
        BankSudo::Mint {
            to_address: admin.to_string(),
            amount: reward_funding.clone(),
        }
    }))
    .unwrap();
    let reward_addr =
        setup_reward_contract(&mut app, staking_addr, Denom::Native(denom), admin.clone());

    app.borrow_mut().update_block(|b| b.height = 1000);

    let fund_msg = ExecuteMsg::Fund {};

    let _res = app
        .borrow_mut()
        .execute_contract(admin, reward_addr.clone(), &fund_msg, &reward_funding)
        .unwrap();

    let res: InfoResponse = app
        .borrow_mut()
        .wrap()
        .query_wasm_smart(&reward_addr, &QueryMsg::Info {})
        .unwrap();

    assert_eq!(res.reward.reward_rate, Uint128::new(10));
    assert_eq!(res.reward.period_finish, 101000);
    assert_eq!(res.reward.reward_duration, 100000);

    app.borrow_mut().update_block(next_block);
    assert_pending_rewards(&mut app, &reward_addr, ADDR1, 5);
    assert_pending_rewards(&mut app, &reward_addr, ADDR2, 2);
    assert_pending_rewards(&mut app, &reward_addr, ADDR3, 2);
}

#[test]
fn test_zero_reward_rate_failed() {
    // This test is due to a bug when funder provides rewards config that results in less then 1
    // reward per block which rounds down to zer0
    let mut app = mock_app();
    let admin = Addr::unchecked(OWNER);
    app.borrow_mut().update_block(|b| b.height = 0);
    let initial_balances = vec![
        Cw20Coin {
            address: ADDR1.to_string(),
            amount: Uint128::new(100),
        },
        Cw20Coin {
            address: ADDR2.to_string(),
            amount: Uint128::new(50),
        },
        Cw20Coin {
            address: ADDR3.to_string(),
            amount: Uint128::new(50),
        },
    ];
    let denom = "utest".to_string();
    let (staking_addr, _) = setup_staking_contract(&mut app, initial_balances);
    let reward_funding = vec![coin(10000, denom.clone())];
    app.sudo(SudoMsg::Bank({
        BankSudo::Mint {
            to_address: admin.to_string(),
            amount: reward_funding.clone(),
        }
    }))
    .unwrap();
    let reward_addr =
        setup_reward_contract(&mut app, staking_addr, Denom::Native(denom), admin.clone());

    app.borrow_mut().update_block(|b| b.height = 1000);

    let fund_msg = ExecuteMsg::Fund {};

    let _res = app
        .borrow_mut()
        .execute_contract(admin, reward_addr, &fund_msg, &reward_funding)
        .unwrap_err();
}

#[test]
fn test_migrate_from_v1() {
    let mut app = App::default();

    let v1_code = app.store_code(cw20_stake_external_rewards_v1_contract());
    let v2_code = app.store_code(cw20_stake_external_rewards_contract());

    let initial_balances = vec![
        Cw20Coin {
            address: ADDR1.to_string(),
            amount: Uint128::new(100),
        },
        Cw20Coin {
            address: ADDR2.to_string(),
            amount: Uint128::new(50),
        },
        Cw20Coin {
            address: ADDR3.to_string(),
            amount: Uint128::new(50),
        },
    ];
    let denom = "utest".to_string();
    let (staking_addr, _) = setup_staking_contract(&mut app, initial_balances);

    let rewards_addr = app
        .instantiate_contract(
            v1_code,
            Addr::unchecked(OWNER),
            &v1::msg::InstantiateMsg {
                owner: Some(OWNER.to_string()),
                manager: Some(ADDR1.to_string()),
                staking_contract: staking_addr.into_string(),
                reward_token: cw20_013::Denom::Native(denom),
                reward_duration: 10000,
            },
            &[],
            "rewards".to_string(),
            Some(OWNER.to_string()),
        )
        .unwrap();

    app.execute(
        Addr::unchecked(OWNER),
        WasmMsg::Migrate {
            contract_addr: rewards_addr.to_string(),
            new_code_id: v2_code,
            msg: to_json_binary(&MigrateMsg::FromV1 {}).unwrap(),
        }
        .into(),
    )
    .unwrap();

    let ownership = get_ownership(&app, &rewards_addr);
    assert_eq!(
        ownership,
        Ownership::<Addr> {
            owner: Some(Addr::unchecked(OWNER)),
            pending_owner: None,
            pending_expiry: None,
        }
    );

    let err: ContractError = app
        .execute(
            Addr::unchecked(OWNER),
            WasmMsg::Migrate {
                contract_addr: rewards_addr.to_string(),
                new_code_id: v2_code,
                msg: to_json_binary(&MigrateMsg::FromV1 {}).unwrap(),
            }
            .into(),
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::AlreadyMigrated {});
}
