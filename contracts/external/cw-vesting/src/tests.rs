use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
use cosmwasm_std::{
    coin, coins, from_binary, to_binary, Addr, Coin, CosmosMsg, Decimal, DistributionMsg, Empty,
    StakingMsg, Timestamp, Uint128, Validator,
};
use cw20::{Cw20Coin, Cw20ExecuteMsg};
use cw_denom::UncheckedDenom;
use cw_multi_test::{
    App, AppBuilder, BankSudo, Contract, ContractWrapper, Executor, StakingInfo, SudoMsg,
};
use cw_ownable::OwnershipError;
use dao_testing::contracts::cw20_base_contract;

use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg, ReceiveMsg};
use crate::vesting::{Schedule, Status, Vest};
use crate::ContractError;

const ALICE: &str = "alice";
const BOB: &str = "bob";
const INITIAL_BALANCE: u128 = 1000000000;
const TOTAL_VEST: u128 = 1000000;
const OWNER: &str = "owner";
const NATIVE_DENOM: &str = "ujuno";
const VALIDATOR: &str = "validator";
const VALIDATOR_TWO: &str = "validator2";

fn cw_vesting_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        crate::contract::execute,
        crate::contract::instantiate,
        crate::contract::query,
    );
    Box::new(contract)
}

fn get_vesting_payment(app: &App, cw_vesting_addr: Addr) -> Vest {
    app.wrap()
        .query_wasm_smart(cw_vesting_addr, &QueryMsg::Vest {})
        .unwrap()
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

pub fn setup_app() -> App {
    let mut app = App::default();

    // Mint Alice and Bob native tokens
    app.sudo(SudoMsg::Bank({
        BankSudo::Mint {
            to_address: ALICE.to_string(),
            amount: coins(INITIAL_BALANCE, NATIVE_DENOM),
        }
    }))
    .unwrap();
    app.sudo(SudoMsg::Bank({
        BankSudo::Mint {
            to_address: BOB.to_string(),
            amount: coins(INITIAL_BALANCE, NATIVE_DENOM),
        }
    }))
    .unwrap();
    app.sudo(SudoMsg::Bank({
        BankSudo::Mint {
            to_address: OWNER.to_string(),
            amount: coins(INITIAL_BALANCE, NATIVE_DENOM),
        }
    }))
    .unwrap();

    app
}

pub fn setup_contracts(app: &mut App) -> (Addr, u64, u64) {
    let cw20_code_id = app.store_code(cw20_base_contract());
    let cw_vesting_code_id = app.store_code(cw_vesting_contract());

    // Instantiate cw20 contract with balances for Alice and Bob
    let cw20_addr = app
        .instantiate_contract(
            cw20_code_id,
            Addr::unchecked(OWNER),
            &cw20_base::msg::InstantiateMsg {
                name: "cw20 token".to_string(),
                symbol: "cwtwenty".to_string(),
                decimals: 6,
                initial_balances: vec![
                    Cw20Coin {
                        address: ALICE.to_string(),
                        amount: Uint128::new(INITIAL_BALANCE),
                    },
                    Cw20Coin {
                        address: BOB.to_string(),
                        amount: Uint128::new(INITIAL_BALANCE),
                    },
                    Cw20Coin {
                        address: OWNER.to_string(),
                        amount: Uint128::new(INITIAL_BALANCE),
                    },
                ],
                mint: None,
                marketing: None,
            },
            &[],
            "cw20-base",
            None,
        )
        .unwrap();

    (cw20_addr, cw20_code_id, cw_vesting_code_id)
}

#[cfg(test)]
impl Default for InstantiateMsg {
    fn default() -> Self {
        Self {
            owner: Some(OWNER.to_string()),
            recipient: BOB.to_string(),
            title: "title".to_string(),
            description: "desc".to_string(),
            total: Uint128::new(TOTAL_VEST),
            // cw20 normally first contract instantaited
            denom: UncheckedDenom::Cw20("contract0".to_string()),
            schedule: Schedule::SaturatingLinear,
            start_time: None,
            vesting_duration_seconds: 604800,    // one week
            unbonding_duration_seconds: 2592000, // 30 days
        }
    }
}

struct TestCase {
    cw20_addr: Addr,
    cw_vesting_addr: Addr,
    recipient: Addr,
    vesting_payment: Vest,
}

fn setup_test_case(app: &mut App, msg: InstantiateMsg, funds: &[Coin]) -> TestCase {
    let (cw20_addr, _, cw_vesting_code_id) = setup_contracts(app);

    // Instantiate cw-vesting contract
    let cw_vesting_addr = app
        .instantiate_contract(
            cw_vesting_code_id,
            Addr::unchecked(OWNER),
            &msg,
            funds,
            "cw-vesting",
            None,
        )
        .unwrap();

    let vesting_payment = match msg.denom {
        UncheckedDenom::Cw20(ref cw20_addr) => {
            let msg = Cw20ExecuteMsg::Send {
                contract: cw_vesting_addr.to_string(),
                amount: msg.total,
                msg: to_binary(&ReceiveMsg::Fund {}).unwrap(),
            };
            app.execute_contract(
                Addr::unchecked(OWNER),
                Addr::unchecked(cw20_addr.clone()),
                &msg,
                &[],
            )
            .unwrap();

            get_vesting_payment(app, cw_vesting_addr.clone())
        }
        UncheckedDenom::Native(_) => get_vesting_payment(app, cw_vesting_addr.clone()),
    };

    TestCase {
        cw20_addr,
        cw_vesting_addr,
        recipient: Addr::unchecked(msg.recipient),
        vesting_payment,
    }
}

#[test]
fn test_happy_cw20_path() {
    let mut app = setup_app();

    let TestCase {
        cw20_addr,
        cw_vesting_addr,
        recipient: bob,
        vesting_payment,
        ..
    } = setup_test_case(&mut app, InstantiateMsg::default(), &[]);

    // Check Vesting Payment was created correctly
    assert_eq!(vesting_payment.status, Status::Funded);
    assert_eq!(vesting_payment.claimed, Uint128::zero());
    assert_eq!(
        vesting_payment.vested(&app.block_info().time),
        Uint128::zero()
    );

    // No time has passed, so nothing is withdrawable.
    let err: ContractError = app
        .execute_contract(
            bob.clone(),
            cw_vesting_addr.clone(),
            &ExecuteMsg::Distribute { amount: None },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(
        err,
        ContractError::InvalidWithdrawal {
            request: Uint128::zero(),
            claimable: Uint128::zero()
        }
    );

    // Advance the clock by 1/2 the vesting period.
    app.update_block(|block| {
        block.time = block.time.plus_seconds(604800 / 2);
    });

    // Distribute, expect to receive 50% of funds.
    app.execute_contract(
        bob,
        cw_vesting_addr,
        &ExecuteMsg::Distribute { amount: None },
        &[],
    )
    .unwrap();

    // Owner has funded the contract and down
    assert_eq!(
        get_balance_cw20(&app, cw20_addr.clone(), OWNER),
        Uint128::new(INITIAL_BALANCE - TOTAL_VEST)
    );

    // Bob has claimed vested funds and is up
    assert_eq!(
        get_balance_cw20(&app, cw20_addr, BOB),
        Uint128::new(INITIAL_BALANCE) + Uint128::new(TOTAL_VEST / 2)
    );
}

#[test]
fn test_happy_native_path() {
    let mut app = setup_app();

    let msg = InstantiateMsg {
        denom: UncheckedDenom::Native(NATIVE_DENOM.to_string()),
        ..Default::default()
    };

    let TestCase {
        cw_vesting_addr,
        recipient: bob,
        vesting_payment,
        ..
    } = setup_test_case(&mut app, msg, &coins(TOTAL_VEST, NATIVE_DENOM));

    assert_eq!(vesting_payment.status, Status::Funded);
    assert_eq!(vesting_payment.claimed, Uint128::zero());
    assert_eq!(
        vesting_payment.vested(&app.block_info().time),
        Uint128::zero()
    );

    // No time has passed, so nothing is withdrawable.
    let err: ContractError = app
        .execute_contract(
            bob.clone(),
            cw_vesting_addr.clone(),
            &ExecuteMsg::Distribute { amount: None },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(
        err,
        ContractError::InvalidWithdrawal {
            request: Uint128::zero(),
            claimable: Uint128::zero()
        }
    );

    // Advance the clock by 1/2 the vesting period.
    app.update_block(|block| {
        block.time = block.time.plus_seconds(604800 / 2);
    });

    // Distribute, expect to receive 50% of funds.
    app.execute_contract(
        bob,
        cw_vesting_addr,
        &ExecuteMsg::Distribute { amount: None },
        &[],
    )
    .unwrap();

    // Owner has funded the contract and down 1000
    assert_eq!(
        get_balance_native(&app, OWNER, NATIVE_DENOM),
        Uint128::new(INITIAL_BALANCE - TOTAL_VEST)
    );
    // Bob has claimed vested funds and is up 250
    assert_eq!(
        get_balance_native(&app, BOB, NATIVE_DENOM),
        Uint128::new(INITIAL_BALANCE) + Uint128::new(TOTAL_VEST / 2)
    );
}

#[test]
fn test_staking_rewards_go_to_receiver() {
    let validator = Validator {
        address: "testvaloper1".to_string(),
        commission: Decimal::percent(1),
        max_commission: Decimal::percent(100),
        max_change_rate: Decimal::percent(1),
    };

    let mut app = AppBuilder::default().build(|router, api, storage| {
        router
            .staking
            .setup(
                storage,
                StakingInfo {
                    bonded_denom: NATIVE_DENOM.to_string(),
                    unbonding_time: 60,
                    /// Interest rate per year (60 * 60 * 24 * 365 seconds)
                    apr: Decimal::percent(10),
                },
            )
            .unwrap();
        router
            .staking
            .add_validator(api, storage, &mock_env().block, validator)
            .unwrap();
    });

    let vesting_id = app.store_code(cw_vesting_contract());
    app.sudo(SudoMsg::Bank(BankSudo::Mint {
        to_address: OWNER.to_string(),
        amount: coins(100, NATIVE_DENOM),
    }))
    .unwrap();

    let msg = InstantiateMsg {
        denom: UncheckedDenom::Native(NATIVE_DENOM.to_string()),
        total: Uint128::new(100),
        ..Default::default()
    };

    let vesting = app
        .instantiate_contract(
            vesting_id,
            Addr::unchecked(OWNER),
            &msg,
            &coins(100, NATIVE_DENOM),
            "cw-vesting",
            None,
        )
        .unwrap();

    // delegate all of the tokens to the validaor.
    app.execute_contract(
        Addr::unchecked(BOB),
        vesting.clone(),
        &ExecuteMsg::Delegate {
            validator: "testvaloper1".to_string(),
            amount: Uint128::new(100),
        },
        &[],
    )
    .unwrap();

    let balance = get_balance_native(&app, BOB, NATIVE_DENOM);
    assert_eq!(balance.u128(), 0);

    // A year passes.
    app.update_block(|block| block.time = block.time.plus_seconds(60 * 60 * 24 * 365));

    app.execute_contract(
        Addr::unchecked(BOB),
        vesting,
        &ExecuteMsg::WithdrawDelegatorReward {
            validator: "testvaloper1".to_string(),
        },
        &[],
    )
    .unwrap();

    let balance = get_balance_native(&app, BOB, NATIVE_DENOM);
    assert_eq!(balance.u128(), 9); // 10% APY, 1% comission, 100 staked, one year elapsed.
}

#[test]
fn test_cancel_vesting() {
    let mut app = setup_app();

    let TestCase {
        cw_vesting_addr, ..
    } = setup_test_case(&mut app, InstantiateMsg::default(), &[]);

    // Non-owner can't cancel
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ALICE),
            cw_vesting_addr.clone(),
            &ExecuteMsg::Cancel {},
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(
        err,
        ContractError::Ownable(cw_ownable::OwnershipError::NotOwner)
    );

    // Advance the clock by 1/2 the vesting period.
    app.update_block(|block| {
        block.time = block.time.plus_seconds(604800 / 2);
    });

    // Owner DAO cancels vesting contract. All tokens are liquid so
    // everything settles instantly.
    app.execute_contract(
        Addr::unchecked(OWNER),
        cw_vesting_addr.clone(),
        &ExecuteMsg::Cancel {},
        &[],
    )
    .unwrap();

    // Can't distribute as tokens are already distributed.
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(BOB),
            cw_vesting_addr,
            &ExecuteMsg::Distribute { amount: None },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert!(matches!(err, ContractError::InvalidWithdrawal { .. }));

    // Unvested funds have been returned to contract owner
    assert_eq!(
        get_balance_cw20(&app, "contract0", OWNER),
        Uint128::new(INITIAL_BALANCE - TOTAL_VEST / 2)
    );
    // Bob has gets the funds vest up until cancelation
    assert_eq!(
        get_balance_cw20(&app, "contract0", BOB),
        Uint128::new(INITIAL_BALANCE + TOTAL_VEST / 2)
    );
}

#[test]
fn staking_unit_tests() {
    use crate::contract::{execute, instantiate, query};

    let mut deps = mock_dependencies();

    // Update staking querier info
    deps.querier.update_staking(NATIVE_DENOM, &[], &[]);

    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(0);
    let alice = mock_info(ALICE, &[]);
    let bob = mock_info(BOB, &[]);

    let unchecked_denom = UncheckedDenom::Native(NATIVE_DENOM.to_string());

    let msg = InstantiateMsg {
        denom: unchecked_denom,
        ..Default::default()
    };

    let amount = Uint128::new(TOTAL_VEST);

    // Alice successfully instantiates
    instantiate(
        deps.as_mut(),
        env.clone(),
        mock_info(ALICE, &coins(TOTAL_VEST, NATIVE_DENOM.to_string())),
        msg,
    )
    .unwrap();

    // let some time pass so that there are payments around.
    env.block.time = Timestamp::from_seconds(5_000);

    // Alice can't delegate his vesting payment
    let err = execute(
        deps.as_mut(),
        env.clone(),
        alice.clone(),
        ExecuteMsg::Delegate {
            validator: VALIDATOR.to_string(),
            amount,
        },
    )
    .unwrap_err();
    assert_eq!(err, ContractError::NotReceiver);

    // Bob delegates his vesting payment
    let res = execute(
        deps.as_mut(),
        env.clone(),
        bob.clone(),
        ExecuteMsg::Delegate {
            validator: VALIDATOR.to_string(),
            amount,
        },
    )
    .unwrap();
    assert_eq!(
        res.messages[0].msg,
        CosmosMsg::Staking(StakingMsg::Delegate {
            validator: VALIDATOR.to_string(),
            amount: coin(amount.into(), NATIVE_DENOM)
        })
    );

    // Any can call Withdraw Rewards, even alice
    let res = execute(
        deps.as_mut(),
        env.clone(),
        alice.clone(),
        ExecuteMsg::WithdrawDelegatorReward {
            validator: VALIDATOR.to_string(),
        },
    )
    .unwrap();
    assert_eq!(
        res.messages[0].msg,
        CosmosMsg::Distribution(DistributionMsg::WithdrawDelegatorReward {
            validator: VALIDATOR.to_string(),
        })
    );

    // Alice can't redelegate or undelegate on bob's behalf
    let err = execute(
        deps.as_mut(),
        env.clone(),
        alice.clone(),
        ExecuteMsg::Redelegate {
            src_validator: VALIDATOR.to_string(),
            dst_validator: VALIDATOR_TWO.to_string(),
            amount,
        },
    )
    .unwrap_err();
    assert_eq!(err, ContractError::NotReceiver);

    let err = execute(
        deps.as_mut(),
        env.clone(),
        alice.clone(),
        ExecuteMsg::Undelegate {
            validator: VALIDATOR.to_string(),
            amount,
        },
    )
    .unwrap_err();
    assert_eq!(err, ContractError::NotReceiver);

    // Bob redelegates half their tokens
    let res = execute(
        deps.as_mut(),
        env.clone(),
        bob.clone(),
        ExecuteMsg::Redelegate {
            src_validator: VALIDATOR.to_string(),
            dst_validator: VALIDATOR_TWO.to_string(),
            amount: amount - amount.checked_div(Uint128::new(2)).unwrap(),
        },
    )
    .unwrap();
    assert_eq!(
        res.messages[0].msg,
        CosmosMsg::Staking(StakingMsg::Redelegate {
            src_validator: VALIDATOR.to_string(),
            dst_validator: VALIDATOR_TWO.to_string(),
            amount: Coin {
                denom: NATIVE_DENOM.to_string(),
                amount: amount - amount.checked_div(Uint128::new(2)).unwrap(),
            }
        })
    );

    // Bob undelegates a little from validator two
    let res = execute(
        deps.as_mut(),
        env.clone(),
        bob.clone(),
        ExecuteMsg::Undelegate {
            validator: VALIDATOR_TWO.to_string(),
            amount: Uint128::new(10),
        },
    )
    .unwrap();
    assert_eq!(
        res.messages[0].msg,
        CosmosMsg::Staking(StakingMsg::Undelegate {
            validator: VALIDATOR_TWO.to_string(),
            amount: coin(10, NATIVE_DENOM)
        })
    );

    // Only Bob (the recipient) can call SetWithdrawAddress
    let err = execute(
        deps.as_mut(),
        env.clone(),
        alice,
        ExecuteMsg::SetWithdrawAddress {
            address: ALICE.to_string(),
        },
    )
    .unwrap_err();
    assert_eq!(err, ContractError::NotReceiver);

    let res = execute(
        deps.as_mut(),
        env.clone(),
        bob.clone(),
        ExecuteMsg::SetWithdrawAddress {
            address: "bob2".to_string(),
        },
    )
    .unwrap();
    assert_eq!(
        res.messages[0].msg,
        CosmosMsg::Distribution(DistributionMsg::SetWithdrawAddress {
            address: "bob2".to_string()
        })
    );

    let owner = mock_info(OWNER, &[]);
    // Contract owner cancels contract, it includes unbonding message
    // for all validators bob delegates to
    execute(
        deps.as_mut(),
        env.clone(),
        owner.clone(),
        ExecuteMsg::Cancel {},
    )
    .unwrap();

    let vest: Vest =
        from_binary(&query(deps.as_ref(), env.clone(), QueryMsg::Vest {}).unwrap()).unwrap();
    assert!(vest.vested(&env.block.time) != vest.claimed);
    // contract is now closed, access checks: both can undelegate,
    // owner can redelegate, nobody can delegate.
    execute(
        deps.as_mut(),
        env.clone(),
        bob.clone(),
        ExecuteMsg::Undelegate {
            validator: VALIDATOR_TWO.to_string(),
            amount: Uint128::new(10),
        },
    )
    .unwrap();
    execute(
        deps.as_mut(),
        env.clone(),
        owner.clone(),
        ExecuteMsg::Undelegate {
            validator: VALIDATOR_TWO.to_string(),
            amount: Uint128::new(10),
        },
    )
    .unwrap();

    execute(
        deps.as_mut(),
        env.clone(),
        owner.clone(),
        ExecuteMsg::Redelegate {
            src_validator: VALIDATOR.to_string(),
            dst_validator: VALIDATOR_TWO.to_string(),
            amount: Uint128::new(10),
        },
    )
    .unwrap();
    let err = execute(
        deps.as_mut(),
        env.clone(),
        bob.clone(),
        ExecuteMsg::Redelegate {
            src_validator: VALIDATOR.to_string(),
            dst_validator: VALIDATOR_TWO.to_string(),
            amount: Uint128::new(10),
        },
    )
    .unwrap_err();
    assert_eq!(err, ContractError::Ownable(OwnershipError::NotOwner));

    let err = execute(
        deps.as_mut(),
        env.clone(),
        bob,
        ExecuteMsg::Delegate {
            validator: VALIDATOR.to_string(),
            amount,
        },
    )
    .unwrap_err();
    assert_eq!(err, ContractError::Cancelled);

    let err = execute(
        deps.as_mut(),
        env,
        owner,
        ExecuteMsg::Delegate {
            validator: VALIDATOR.to_string(),
            amount,
        },
    )
    .unwrap_err();
    assert_eq!(err, ContractError::Cancelled);
}

#[test]
fn test_catch_imposter_cw20() {
    let mut app = setup_app();
    let (_, cw20_code_id, _) = setup_contracts(&mut app);

    let TestCase {
        cw_vesting_addr, ..
    } = setup_test_case(&mut app, InstantiateMsg::default(), &[]);

    // Create imposter cw20
    let cw20_imposter_addr = app
        .instantiate_contract(
            cw20_code_id,
            Addr::unchecked(OWNER),
            &cw20_base::msg::InstantiateMsg {
                name: "cw20 token".to_string(),
                symbol: "cwtwenty".to_string(),
                decimals: 6,
                initial_balances: vec![Cw20Coin {
                    address: OWNER.to_string(),
                    amount: Uint128::new(INITIAL_BALANCE),
                }],
                mint: None,
                marketing: None,
            },
            &[],
            "cw20-base",
            None,
        )
        .unwrap();

    let msg = Cw20ExecuteMsg::Send {
        contract: cw_vesting_addr.to_string(),
        amount: Uint128::new(TOTAL_VEST),
        msg: to_binary(&ReceiveMsg::Fund {}).unwrap(),
    };

    // Errors that cw20 does not match what was expected
    let error: ContractError = app
        .execute_contract(
            Addr::unchecked(OWNER),
            Addr::unchecked(cw20_imposter_addr),
            &msg,
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(error, ContractError::WrongCw20);
}

#[test]
fn test_incorrect_native_funding_amount() {
    let mut app = setup_app();

    let unchecked_denom = UncheckedDenom::Native(NATIVE_DENOM.to_string());

    let msg = InstantiateMsg {
        denom: unchecked_denom,
        ..Default::default()
    };

    let alice = Addr::unchecked(ALICE);

    let (_, _, cw_vesting_code_id) = setup_contracts(&mut app);

    // Instantiate cw-vesting contract errors with incorrect amount
    let error: ContractError = app
        .instantiate_contract(
            cw_vesting_code_id,
            alice,
            &msg,
            &coins(100, NATIVE_DENOM),
            "cw-vesting",
            None,
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(
        error,
        ContractError::WrongFundAmount {
            sent: Uint128::new(100),
            expected: Uint128::new(TOTAL_VEST)
        }
    )
}
