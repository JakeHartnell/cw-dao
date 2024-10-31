use cosmwasm_std::{coins, Addr, Uint128};
use cw20::Cw20Coin;
use cw_multi_test::{App, BankSudo, Executor};
use dao_testing::contracts::cw20_base_contract;

use crate::CheckedDenom;

#[test]
fn test_cw20_denom_send() {
    let mut app = App::default();

    let cw20_id = app.store_code(cw20_base_contract());
    let cw20 = app
        .instantiate_contract(
            cw20_id,
            Addr::unchecked("ekez"),
            &cw20_base::msg::InstantiateMsg {
                name: "token".to_string(),
                symbol: "symbol".to_string(),
                decimals: 6,
                initial_balances: vec![Cw20Coin {
                    amount: Uint128::new(10),
                    address: "ekez".to_string(),
                }],
                mint: None,
                marketing: None,
            },
            &[],
            "token contract",
            None,
        )
        .unwrap();

    let denom = CheckedDenom::Cw20(cw20);

    let start_balance = denom
        .query_balance(&app.wrap(), &Addr::unchecked("ekez"))
        .unwrap();
    let send_message = denom
        .get_transfer_to_message(&Addr::unchecked("dao"), Uint128::new(9))
        .unwrap();
    app.execute(Addr::unchecked("ekez"), send_message).unwrap();
    let end_balance = denom
        .query_balance(&app.wrap(), &Addr::unchecked("ekez"))
        .unwrap();

    assert_eq!(start_balance, Uint128::new(10));
    assert_eq!(end_balance, Uint128::new(1));

    let dao_balance = denom
        .query_balance(&app.wrap(), &Addr::unchecked("dao"))
        .unwrap();
    assert_eq!(dao_balance, Uint128::new(9))
}

#[test]
fn test_native_denom_send() {
    let mut app = App::default();
    app.sudo(cw_multi_test::SudoMsg::Bank(BankSudo::Mint {
        to_address: "ekez".to_string(),
        amount: coins(10, "ujuno"),
    }))
    .unwrap();

    let denom = CheckedDenom::Native("ujuno".to_string());

    let start_balance = denom
        .query_balance(&app.wrap(), &Addr::unchecked("ekez"))
        .unwrap();
    let send_message = denom
        .get_transfer_to_message(&Addr::unchecked("dao"), Uint128::new(9))
        .unwrap();
    app.execute(Addr::unchecked("ekez"), send_message).unwrap();
    let end_balance = denom
        .query_balance(&app.wrap(), &Addr::unchecked("ekez"))
        .unwrap();

    assert_eq!(start_balance, Uint128::new(10));
    assert_eq!(end_balance, Uint128::new(1));

    let dao_balance = denom
        .query_balance(&app.wrap(), &Addr::unchecked("dao"))
        .unwrap();
    assert_eq!(dao_balance, Uint128::new(9))
}
