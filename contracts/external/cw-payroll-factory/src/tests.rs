use cosmwasm_std::{coins, to_json_binary, Addr, Uint128};
use cw20::{Cw20Coin, Cw20ExecuteMsg};
use cw_denom::UncheckedDenom;
use cw_multi_test::{App, BankSudo, Executor, SudoMsg};
use cw_ownable::OwnershipError;
use cw_vesting::{
    msg::{InstantiateMsg as PayrollInstantiateMsg, QueryMsg as PayrollQueryMsg},
    vesting::{Schedule, Status, Vest},
};
use dao_testing::contracts::{
    cw20_base_contract, cw_payroll_factory_contract, cw_vesting_contract,
};

use crate::{
    msg::{ExecuteMsg, InstantiateMsg, QueryMsg, ReceiveMsg},
    state::VestingContract,
};
use cw_payroll_factory::ContractError;

const ALICE: &str = "alice";
const BOB: &str = "bob";
const INITIAL_BALANCE: u128 = 1000000000;
const NATIVE_DENOM: &str = "denom";

#[test]
pub fn test_instantiate_native_payroll_contract() {
    let mut app = App::default();
    let code_id = app.store_code(cw_payroll_factory_contract());
    let cw_vesting_code_id = app.store_code(cw_vesting_contract());

    // Instantiate factory with only Alice allowed to instantiate payroll contracts
    let instantiate = InstantiateMsg {
        owner: Some(ALICE.to_string()),
        vesting_code_id: cw_vesting_code_id,
    };
    let factory_addr = app
        .instantiate_contract(
            code_id,
            Addr::unchecked("CREATOR"),
            &instantiate,
            &[],
            "cw-admin-factory",
            None,
        )
        .unwrap();

    // Mint alice and bob native tokens
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

    let amount = Uint128::new(1000000);
    let unchecked_denom = UncheckedDenom::Native(NATIVE_DENOM.to_string());

    let instantiate_payroll_msg = ExecuteMsg::InstantiateNativePayrollContract {
        instantiate_msg: PayrollInstantiateMsg {
            owner: Some(ALICE.to_string()),
            recipient: BOB.to_string(),
            title: "title".to_string(),
            description: Some("desc".to_string()),
            total: amount,
            denom: unchecked_denom,
            schedule: Schedule::SaturatingLinear,
            vesting_duration_seconds: 200,
            unbonding_duration_seconds: 2592000, // 30 days
            start_time: None,
        },
        label: "Payroll".to_string(),
    };

    let res = app
        .execute_contract(
            Addr::unchecked(ALICE),
            factory_addr.clone(),
            &instantiate_payroll_msg,
            &coins(amount.into(), NATIVE_DENOM),
        )
        .unwrap();

    // BOB can't instantiate as owner is configured
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(BOB),
            factory_addr.clone(),
            &instantiate_payroll_msg,
            &coins(amount.into(), NATIVE_DENOM),
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::Unauthorized {});

    // Get the payroll address from the instantiate event
    let instantiate_event = &res.events[2];
    assert_eq!(instantiate_event.ty, "instantiate");
    let cw_vesting_addr = instantiate_event.attributes[0].value.clone();

    // Check that admin of contract is owner specified in Instantiation Message
    let contract_info = app
        .wrap()
        .query_wasm_contract_info(cw_vesting_addr)
        .unwrap();
    assert_eq!(contract_info.admin, Some(ALICE.to_string()));

    // Test query list of contracts
    let contracts: Vec<VestingContract> = app
        .wrap()
        .query_wasm_smart(
            factory_addr.clone(),
            &QueryMsg::ListVestingContracts {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(contracts.len(), 1);

    // Test query by instantiator
    let contracts: Vec<VestingContract> = app
        .wrap()
        .query_wasm_smart(
            factory_addr.clone(),
            &QueryMsg::ListVestingContractsByInstantiator {
                instantiator: ALICE.to_string(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(contracts.len(), 1);

    // Test query by instantiator with no results
    let contracts: Vec<VestingContract> = app
        .wrap()
        .query_wasm_smart(
            factory_addr.clone(),
            &QueryMsg::ListVestingContractsByInstantiator {
                instantiator: BOB.to_string(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(contracts.len(), 0);

    // Test query by recipient
    let contracts: Vec<VestingContract> = app
        .wrap()
        .query_wasm_smart(
            factory_addr.clone(),
            &QueryMsg::ListVestingContractsByRecipient {
                recipient: BOB.to_string(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(contracts.len(), 1);

    // Test query by recipient no results
    let contracts: Vec<VestingContract> = app
        .wrap()
        .query_wasm_smart(
            factory_addr,
            &QueryMsg::ListVestingContractsByRecipient {
                recipient: ALICE.to_string(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(contracts.len(), 0);
}

#[test]
pub fn test_instantiate_cw20_payroll_contract() {
    let mut app = App::default();
    let code_id = app.store_code(cw_payroll_factory_contract());
    let cw20_code_id = app.store_code(cw20_base_contract());
    let cw_vesting_code_id = app.store_code(cw_vesting_contract());

    // Instantiate cw20 contract with balances for Alice
    let cw20_addr = app
        .instantiate_contract(
            cw20_code_id,
            Addr::unchecked(ALICE),
            &cw20_base::msg::InstantiateMsg {
                name: "cw20 token".to_string(),
                symbol: "cwtwenty".to_string(),
                decimals: 6,
                initial_balances: vec![Cw20Coin {
                    address: ALICE.to_string(),
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

    let instantiate = InstantiateMsg {
        owner: Some(ALICE.to_string()),
        vesting_code_id: cw_vesting_code_id,
    };
    let factory_addr = app
        .instantiate_contract(
            code_id,
            Addr::unchecked("CREATOR"),
            &instantiate,
            &[],
            "cw-admin-factory",
            None,
        )
        .unwrap();

    // Mint alice native tokens
    app.sudo(SudoMsg::Bank({
        BankSudo::Mint {
            to_address: ALICE.to_string(),
            amount: coins(INITIAL_BALANCE, NATIVE_DENOM),
        }
    }))
    .unwrap();

    let amount = Uint128::new(1000000);
    let unchecked_denom = UncheckedDenom::Cw20(cw20_addr.to_string());

    let instantiate_payroll_msg = PayrollInstantiateMsg {
        owner: Some(ALICE.to_string()),
        recipient: BOB.to_string(),
        title: "title".to_string(),
        description: Some("desc".to_string()),
        total: amount,
        denom: unchecked_denom,
        schedule: Schedule::SaturatingLinear,
        vesting_duration_seconds: 200,
        unbonding_duration_seconds: 2592000, // 30 days
        start_time: None,
    };

    // Attempting to call InstantiatePayrollContract directly with cw20 fails
    app.execute_contract(
        Addr::unchecked(ALICE),
        factory_addr.clone(),
        &ExecuteMsg::InstantiateNativePayrollContract {
            instantiate_msg: instantiate_payroll_msg.clone(),
            label: "Payroll".to_string(),
        },
        &coins(amount.into(), NATIVE_DENOM),
    )
    .unwrap_err();

    let res = app
        .execute_contract(
            Addr::unchecked(ALICE),
            cw20_addr,
            &Cw20ExecuteMsg::Send {
                contract: factory_addr.to_string(),
                amount: instantiate_payroll_msg.total,
                msg: to_json_binary(&ReceiveMsg::InstantiatePayrollContract {
                    instantiate_msg: instantiate_payroll_msg,
                    label: "Payroll".to_string(),
                })
                .unwrap(),
            },
            &coins(amount.into(), NATIVE_DENOM),
        )
        .unwrap();

    // Get the payroll address from the instantiate event
    let instantiate_event = &res.events[4];
    assert_eq!(instantiate_event.ty, "instantiate");
    let cw_vesting_addr = instantiate_event.attributes[0].value.clone();

    // Check that admin of contract is owner specified in Instantiation Message
    let contract_info = app
        .wrap()
        .query_wasm_contract_info(cw_vesting_addr.clone())
        .unwrap();
    assert_eq!(contract_info.admin, Some(ALICE.to_string()));

    // Test query by instantiator
    let contracts: Vec<VestingContract> = app
        .wrap()
        .query_wasm_smart(
            factory_addr,
            &QueryMsg::ListVestingContractsByInstantiator {
                instantiator: ALICE.to_string(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(contracts.len(), 1);

    // Check that the vesting payment contract is active
    let vp: Vest = app
        .wrap()
        .query_wasm_smart(cw_vesting_addr, &PayrollQueryMsg::Info {})
        .unwrap();
    assert_eq!(vp.status, Status::Funded);
}

#[test]
fn test_instantiate_wrong_ownership_native() {
    let mut app = App::default();
    let code_id = app.store_code(cw_payroll_factory_contract());
    let cw_vesting_code_id = app.store_code(cw_vesting_contract());

    let amount = Uint128::new(1000000);
    let unchecked_denom = UncheckedDenom::Native(NATIVE_DENOM.to_string());

    app.sudo(SudoMsg::Bank({
        BankSudo::Mint {
            to_address: "ekez".to_string(),
            amount: coins(amount.u128() * 2, NATIVE_DENOM),
        }
    }))
    .unwrap();
    app.sudo(SudoMsg::Bank({
        BankSudo::Mint {
            to_address: ALICE.to_string(),
            amount: coins(amount.u128() * 2, NATIVE_DENOM),
        }
    }))
    .unwrap();

    // Alice is the owner. Contracts are only allowed if their owner
    // is alice or none and the sender is alice.
    let instantiate = InstantiateMsg {
        owner: Some(ALICE.to_string()),
        vesting_code_id: cw_vesting_code_id,
    };
    let factory_addr = app
        .instantiate_contract(
            code_id,
            Addr::unchecked("CREATOR"),
            &instantiate,
            &[],
            "cw-admin-factory",
            None,
        )
        .unwrap();

    let err: ContractError = app
        .execute_contract(
            Addr::unchecked("ekez"),
            factory_addr,
            &ExecuteMsg::InstantiateNativePayrollContract {
                instantiate_msg: PayrollInstantiateMsg {
                    owner: Some(ALICE.to_string()),
                    recipient: BOB.to_string(),
                    title: "title".to_string(),
                    description: Some("desc".to_string()),
                    total: amount,
                    denom: unchecked_denom,
                    schedule: Schedule::SaturatingLinear,
                    vesting_duration_seconds: 200,
                    unbonding_duration_seconds: 2592000, // 30 days
                    start_time: None,
                },
                label: "vesting".to_string(),
            },
            &coins(amount.u128(), NATIVE_DENOM),
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    // Can't instantiate if you are not the owner.
    assert_eq!(err, ContractError::Unauthorized {});
}

#[test]
fn test_update_vesting_code_id() {
    let mut app = App::default();
    let code_id = app.store_code(cw_payroll_factory_contract());
    let cw_vesting_code_id = app.store_code(cw_vesting_contract());
    let cw_vesting_code_two = app.store_code(cw_vesting_contract());

    // Instantiate factory with only Alice allowed to instantiate payroll contracts
    let instantiate = InstantiateMsg {
        owner: Some(ALICE.to_string()),
        vesting_code_id: cw_vesting_code_id,
    };
    let factory_addr = app
        .instantiate_contract(
            code_id,
            Addr::unchecked("CREATOR"),
            &instantiate,
            &[],
            "cw-admin-factory",
            None,
        )
        .unwrap();

    // Update the code ID to a new one.
    app.execute_contract(
        Addr::unchecked(ALICE),
        factory_addr.clone(),
        &ExecuteMsg::UpdateCodeId {
            vesting_code_id: cw_vesting_code_two,
        },
        &[],
    )
    .unwrap();

    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(BOB),
            factory_addr.clone(),
            &ExecuteMsg::UpdateCodeId {
                vesting_code_id: cw_vesting_code_two,
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::Ownable(OwnershipError::NotOwner));

    app.sudo(SudoMsg::Bank({
        BankSudo::Mint {
            to_address: ALICE.to_string(),
            amount: coins(INITIAL_BALANCE, NATIVE_DENOM),
        }
    }))
    .unwrap();

    let amount = Uint128::new(1000000);
    let unchecked_denom = UncheckedDenom::Native(NATIVE_DENOM.to_string());

    let instantiate_payroll_msg = ExecuteMsg::InstantiateNativePayrollContract {
        instantiate_msg: PayrollInstantiateMsg {
            owner: Some(ALICE.to_string()),
            recipient: BOB.to_string(),
            title: "title".to_string(),
            description: Some("desc".to_string()),
            total: amount,
            denom: unchecked_denom,
            schedule: Schedule::SaturatingLinear,
            vesting_duration_seconds: 200,
            unbonding_duration_seconds: 2592000, // 30 days
            start_time: None,
        },
        label: "Payroll".to_string(),
    };

    let res = app
        .execute_contract(
            Addr::unchecked(ALICE),
            factory_addr,
            &instantiate_payroll_msg,
            &coins(amount.into(), NATIVE_DENOM),
        )
        .unwrap();

    // Check that the contract was instantiated using the new code ID.
    let instantiate_event = &res.events[2];
    assert_eq!(instantiate_event.ty, "instantiate");
    let cw_vesting_addr = instantiate_event.attributes[0].value.clone();
    let info = app
        .wrap()
        .query_wasm_contract_info(cw_vesting_addr)
        .unwrap();
    assert_eq!(info.code_id, cw_vesting_code_two);
}

/// This test was contributed by Oak Security as part of their audit
/// of cw-vesting. It addresses issue two, "Misconfiguring the total
/// vested amount to be lower than the sent CW20 amount would cause a
/// loss of funds".
#[test]
pub fn test_inconsistent_cw20_amount() {
    let mut app = App::default();
    let code_id = app.store_code(cw_payroll_factory_contract());
    let cw20_code_id = app.store_code(cw20_base_contract());
    let cw_vesting_code_id = app.store_code(cw_vesting_contract());
    // Instantiate cw20 contract with balances for Alice
    let cw20_addr = app
        .instantiate_contract(
            cw20_code_id,
            Addr::unchecked(ALICE),
            &cw20_base::msg::InstantiateMsg {
                name: "cw20 token".to_string(),
                symbol: "cwtwenty".to_string(),
                decimals: 6,
                initial_balances: vec![Cw20Coin {
                    address: ALICE.to_string(),
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
    let instantiate = InstantiateMsg {
        owner: Some(ALICE.to_string()),
        vesting_code_id: cw_vesting_code_id,
    };
    let factory_addr = app
        .instantiate_contract(
            code_id,
            Addr::unchecked("CREATOR"),
            &instantiate,
            &[],
            "cw-admin-factory",
            None,
        )
        .unwrap();
    // Mint alice native tokens
    app.sudo(SudoMsg::Bank({
        BankSudo::Mint {
            to_address: ALICE.to_string(),
            amount: coins(INITIAL_BALANCE, NATIVE_DENOM),
        }
    }))
    .unwrap();
    let amount = Uint128::new(1000000);
    let unchecked_denom = UncheckedDenom::Cw20(cw20_addr.to_string());
    let instantiate_payroll_msg = PayrollInstantiateMsg {
        owner: Some(ALICE.to_string()),
        recipient: BOB.to_string(),
        title: "title".to_string(),
        description: Some("desc".to_string()),
        total: amount - Uint128::new(1), // lesser amount than sent
        denom: unchecked_denom,
        schedule: Schedule::SaturatingLinear,
        vesting_duration_seconds: 200,
        unbonding_duration_seconds: 2592000, // 30 days
        start_time: None,
    };
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(ALICE),
            cw20_addr,
            &Cw20ExecuteMsg::Send {
                contract: factory_addr.to_string(),
                amount,
                msg: to_json_binary(&ReceiveMsg::InstantiatePayrollContract {
                    instantiate_msg: instantiate_payroll_msg,
                    label: "Payroll".to_string(),
                })
                .unwrap(),
            },
            &coins(amount.into(), NATIVE_DENOM), // https://github.com/CosmWasm/cw-plus/issues/862
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(
        err,
        ContractError::WrongFundAmount {
            sent: amount,
            expected: amount - Uint128::one()
        }
    );
}
