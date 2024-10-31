use crate::msg::ExecuteMsg::ClaimAll;
use crate::msg::{ExecuteMsg, InstantiateMsg};
use cosmwasm_std::{to_json_binary, Addr, Binary, Coin, Uint128};
use cw20::{BalanceResponse, Cw20Coin};
use cw_multi_test::{next_block, App, BankSudo, Executor, SudoMsg};
use cw_utils::Duration;
use dao_testing::contracts::{
    cw20_base_contract, cw20_stake_contract, cw_fund_distributor_contract,
    dao_voting_cw20_staked_contract,
};

const CREATOR_ADDR: &str = "creator";
const FEE_DENOM: &str = "ujuno";

struct BaseTest {
    app: App,
    distributor_address: Addr,
}

fn instantiate_cw20(
    app: &mut App,
    sender: Addr,
    initial_balances: Vec<Cw20Coin>,
    name: String,
    symbol: String,
) -> Addr {
    let cw20_id = app.store_code(cw20_base_contract());
    let msg = cw20_base::msg::InstantiateMsg {
        name,
        symbol,
        decimals: 6,
        initial_balances,
        mint: None,
        marketing: None,
    };

    app.instantiate_contract(cw20_id, sender, &msg, &[], "cw20", None)
        .unwrap()
}

fn setup_test(initial_balances: Vec<Cw20Coin>) -> BaseTest {
    let mut app = App::default();
    let distributor_id = app.store_code(cw_fund_distributor_contract());
    let cw20_id = app.store_code(cw20_base_contract());
    let voting_id = app.store_code(dao_voting_cw20_staked_contract());
    let stake_cw20_id = app.store_code(cw20_stake_contract());

    let voting_address = app
        .instantiate_contract(
            voting_id,
            Addr::unchecked(CREATOR_ADDR),
            &dao_voting_cw20_staked::msg::InstantiateMsg {
                active_threshold: None,
                token_info: dao_voting_cw20_staked::msg::TokenInfo::New {
                    code_id: cw20_id,
                    label: "DAO DAO governance token.".to_string(),
                    name: "DAO DAO".to_string(),
                    symbol: "DAO".to_string(),
                    decimals: 6,
                    initial_balances: initial_balances.clone(),
                    marketing: None,
                    staking_code_id: stake_cw20_id,
                    unstaking_duration: None,
                    initial_dao_balance: None,
                },
            },
            &[],
            "voting contract",
            None,
        )
        .unwrap();

    let staking_contract: Addr = app
        .wrap()
        .query_wasm_smart(
            voting_address.clone(),
            &dao_voting_cw20_staked::msg::QueryMsg::StakingContract {},
        )
        .unwrap();

    let token_contract: Addr = app
        .wrap()
        .query_wasm_smart(
            voting_address.clone(),
            &dao_voting_cw20_staked::msg::QueryMsg::TokenContract {},
        )
        .unwrap();

    for Cw20Coin { address, amount } in initial_balances {
        app.execute_contract(
            Addr::unchecked(address),
            token_contract.clone(),
            &cw20_base::msg::ExecuteMsg::Send {
                contract: staking_contract.to_string(),
                amount,
                msg: to_json_binary(&cw20_stake::msg::ReceiveMsg::Stake {}).unwrap(),
            },
            &[],
        )
        .unwrap();
    }

    app.update_block(next_block);

    let distribution_contract = app
        .instantiate_contract(
            distributor_id,
            Addr::unchecked(CREATOR_ADDR),
            &InstantiateMsg {
                voting_contract: voting_address.to_string(),
                funding_period: Duration::Height(10),
                distribution_height: app.block_info().height,
            },
            &[],
            "distribution contract",
            Some(CREATOR_ADDR.parse().unwrap()),
        )
        .unwrap();

    BaseTest {
        app,
        distributor_address: distribution_contract,
    }
}

// This is to attempt to simulate a situation where
// someone would spam a dao treasury with a lot of native tokens
#[test]
pub fn test_claim_lots_of_native_tokens() {
    let BaseTest {
        mut app,
        distributor_address,
    } = setup_test(vec![
        Cw20Coin {
            address: "bekauz".to_string(),
            amount: Uint128::new(10),
        },
        Cw20Coin {
            address: "ekez".to_string(),
            amount: Uint128::new(20),
        },
    ]);

    let amount = Uint128::new(500000);

    let token_count = 500;
    // mint and fund the distributor contract with
    // a bunch of native tokens
    for n in 1..token_count {
        let denom = FEE_DENOM.to_owned() + &n.to_string();
        app.sudo(SudoMsg::Bank(BankSudo::Mint {
            to_address: CREATOR_ADDR.to_string(),
            amount: vec![Coin {
                amount,
                denom: denom.clone(),
            }],
        }))
        .unwrap();

        app.execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            distributor_address.clone(),
            &ExecuteMsg::FundNative {},
            &[Coin {
                amount,
                denom: denom.clone(),
            }],
        )
        .unwrap();
    }

    app.update_block(|block| block.height += 11);

    app.execute_contract(
        Addr::unchecked("bekauz"),
        distributor_address,
        &ClaimAll {},
        &[],
    )
    .unwrap();

    // assert that all the claims succeeded
    for n in 1..token_count {
        let denom = FEE_DENOM.to_owned() + &n.to_string();
        let expected_balance = Uint128::new(166666);
        let user_balance_after_claim = app
            .wrap()
            .query_balance("bekauz".to_string(), denom)
            .unwrap();
        assert_eq!(expected_balance, user_balance_after_claim.amount);
    }
}

// This is to attempt to simulate a situation where
// the distributor contract gets funded with a lot
// of cw20 tokens
#[test]
pub fn test_claim_lots_of_cw20s() {
    let BaseTest {
        mut app,
        distributor_address,
    } = setup_test(vec![
        Cw20Coin {
            address: "bekauz".to_string(),
            amount: Uint128::new(10),
        },
        Cw20Coin {
            address: "ekez".to_string(),
            amount: Uint128::new(20),
        },
    ]);

    let amount = Uint128::new(500000);

    // mint and fund (spam) the distributor contract with
    // a bunch of tokens
    let cw20_addresses: Vec<Addr> = (1..1000)
        .map(|n| {
            let name = FEE_DENOM.to_owned() + &n.to_string();
            let cw20_addr = instantiate_cw20(
                &mut app,
                Addr::unchecked(CREATOR_ADDR),
                vec![Cw20Coin {
                    address: CREATOR_ADDR.to_string(),
                    amount,
                }],
                name,
                "shitcoin".to_string(),
            );
            app.execute_contract(
                Addr::unchecked(CREATOR_ADDR),
                cw20_addr.clone(),
                &cw20::Cw20ExecuteMsg::Send {
                    contract: distributor_address.to_string(),
                    amount,
                    msg: Binary::default(),
                },
                &[],
            )
            .unwrap();
            cw20_addr
        })
        .collect();

    app.update_block(|block| block.height += 11);

    app.execute_contract(
        Addr::unchecked("bekauz"),
        distributor_address,
        &ClaimAll {},
        &[],
    )
    .unwrap();

    let expected_balance = Uint128::new(166666);

    // assert that all the claims succeeded
    cw20_addresses.into_iter().for_each(|addr| {
        let user_balance_after_claim: BalanceResponse = app
            .wrap()
            .query_wasm_smart(
                addr,
                &cw20::Cw20QueryMsg::Balance {
                    address: "bekauz".to_string(),
                },
            )
            .unwrap();
        assert_eq!(expected_balance, user_balance_after_claim.balance);
    });
}
