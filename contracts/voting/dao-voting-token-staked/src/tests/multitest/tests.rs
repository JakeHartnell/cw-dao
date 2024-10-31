use crate::contract::{migrate, CONTRACT_NAME, CONTRACT_VERSION};
use crate::msg::{
    ExecuteMsg, GetHooksResponse, InstantiateMsg, ListStakersResponse, MigrateMsg, QueryMsg,
    StakerBalanceResponse, TokenInfo,
};
use crate::state::Config;
use cosmwasm_std::testing::{mock_dependencies, mock_env};
use cosmwasm_std::{coins, Addr, Coin, Decimal, Uint128};
use cw_controllers::ClaimsResponse;
use cw_multi_test::{next_block, App, AppResponse, BankSudo, Executor, SudoMsg};
use cw_utils::Duration;
use dao_interface::voting::{
    DenomResponse, InfoResponse, IsActiveResponse, TotalPowerAtHeightResponse,
    VotingPowerAtHeightResponse,
};
use dao_testing::contracts::{
    dao_proposal_hook_counter_contract, dao_voting_token_staked_contract,
};
use dao_voting::threshold::{ActiveThreshold, ActiveThresholdResponse};

const DAO_ADDR: &str = "dao";
const ADDR1: &str = "addr1";
const ADDR2: &str = "addr2";
const DENOM: &str = "ujuno";
const INVALID_DENOM: &str = "uinvalid";
const ODD_DENOM: &str = "uodd";

fn mock_app() -> App {
    let mut app = App::default();
    app.sudo(SudoMsg::Bank(BankSudo::Mint {
        to_address: DAO_ADDR.to_string(),
        amount: vec![
            Coin {
                denom: DENOM.to_string(),
                amount: Uint128::new(10000),
            },
            Coin {
                denom: INVALID_DENOM.to_string(),
                amount: Uint128::new(10000),
            },
        ],
    }))
    .unwrap();
    app.sudo(SudoMsg::Bank(BankSudo::Mint {
        to_address: ADDR1.to_string(),
        amount: vec![
            Coin {
                denom: DENOM.to_string(),
                amount: Uint128::new(10000),
            },
            Coin {
                denom: INVALID_DENOM.to_string(),
                amount: Uint128::new(10000),
            },
            Coin {
                denom: ODD_DENOM.to_string(),
                amount: Uint128::new(5),
            },
        ],
    }))
    .unwrap();
    app.sudo(SudoMsg::Bank(BankSudo::Mint {
        to_address: ADDR2.to_string(),
        amount: vec![
            Coin {
                denom: DENOM.to_string(),
                amount: Uint128::new(10000),
            },
            Coin {
                denom: INVALID_DENOM.to_string(),
                amount: Uint128::new(10000),
            },
        ],
    }))
    .unwrap();
    app
}

fn instantiate_staking(app: &mut App, staking_id: u64, msg: InstantiateMsg) -> Addr {
    app.instantiate_contract(
        staking_id,
        Addr::unchecked(DAO_ADDR),
        &msg,
        &[],
        "Staking",
        None,
    )
    .unwrap()
}

fn stake_tokens(
    app: &mut App,
    staking_addr: Addr,
    sender: &str,
    amount: u128,
    denom: &str,
) -> anyhow::Result<AppResponse> {
    app.execute_contract(
        Addr::unchecked(sender),
        staking_addr,
        &ExecuteMsg::Stake {},
        &coins(amount, denom),
    )
}

fn unstake_tokens(
    app: &mut App,
    staking_addr: Addr,
    sender: &str,
    amount: u128,
) -> anyhow::Result<AppResponse> {
    app.execute_contract(
        Addr::unchecked(sender),
        staking_addr,
        &ExecuteMsg::Unstake {
            amount: Uint128::new(amount),
        },
        &[],
    )
}

fn claim(app: &mut App, staking_addr: Addr, sender: &str) -> anyhow::Result<AppResponse> {
    app.execute_contract(
        Addr::unchecked(sender),
        staking_addr,
        &ExecuteMsg::Claim {},
        &[],
    )
}

fn update_config(
    app: &mut App,
    staking_addr: Addr,
    sender: &str,
    duration: Option<Duration>,
) -> anyhow::Result<AppResponse> {
    app.execute_contract(
        Addr::unchecked(sender),
        staking_addr,
        &ExecuteMsg::UpdateConfig { duration },
        &[],
    )
}

fn get_voting_power_at_height(
    app: &mut App,
    staking_addr: Addr,
    address: String,
    height: Option<u64>,
) -> VotingPowerAtHeightResponse {
    app.wrap()
        .query_wasm_smart(
            staking_addr,
            &QueryMsg::VotingPowerAtHeight { address, height },
        )
        .unwrap()
}

fn get_total_power_at_height(
    app: &mut App,
    staking_addr: Addr,
    height: Option<u64>,
) -> TotalPowerAtHeightResponse {
    app.wrap()
        .query_wasm_smart(staking_addr, &QueryMsg::TotalPowerAtHeight { height })
        .unwrap()
}

fn get_config(app: &mut App, staking_addr: Addr) -> Config {
    app.wrap()
        .query_wasm_smart(staking_addr, &QueryMsg::GetConfig {})
        .unwrap()
}

fn get_claims(app: &mut App, staking_addr: Addr, address: String) -> ClaimsResponse {
    app.wrap()
        .query_wasm_smart(staking_addr, &QueryMsg::Claims { address })
        .unwrap()
}

fn get_balance(app: &mut App, address: &str, denom: &str) -> Uint128 {
    app.wrap().query_balance(address, denom).unwrap().amount
}

#[test]
fn test_instantiate_existing() {
    let mut app = mock_app();

    let staking_id = app.store_code(dao_voting_token_staked_contract());
    // Populated fields
    instantiate_staking(
        &mut app,
        staking_id,
        InstantiateMsg {
            token_info: TokenInfo::Existing {
                denom: DENOM.to_string(),
            },
            unstaking_duration: Some(Duration::Height(5)),
            active_threshold: None,
        },
    );

    // Non populated fields
    let addr = instantiate_staking(
        &mut app,
        staking_id,
        InstantiateMsg {
            token_info: TokenInfo::Existing {
                denom: DENOM.to_string(),
            },
            unstaking_duration: None,
            active_threshold: None,
        },
    );

    let denom: DenomResponse = app
        .wrap()
        .query_wasm_smart(addr, &QueryMsg::Denom {})
        .unwrap();
    assert_eq!(
        denom,
        DenomResponse {
            denom: DENOM.to_string()
        }
    );
}

#[test]
#[should_panic(expected = "Invalid unstaking duration, unstaking duration cannot be 0")]
fn test_instantiate_invalid_unstaking_duration_height() {
    let mut app = mock_app();

    let staking_id = app.store_code(dao_voting_token_staked_contract());

    // Populated fields
    instantiate_staking(
        &mut app,
        staking_id,
        InstantiateMsg {
            token_info: TokenInfo::Existing {
                denom: DENOM.to_string(),
            },
            unstaking_duration: Some(Duration::Height(0)),
            active_threshold: Some(ActiveThreshold::AbsoluteCount {
                count: Uint128::new(1),
            }),
        },
    );
}

#[test]
#[should_panic(expected = "Invalid unstaking duration, unstaking duration cannot be 0")]
fn test_instantiate_invalid_unstaking_duration_time() {
    let mut app = mock_app();

    let staking_id = app.store_code(dao_voting_token_staked_contract());

    // Populated fields with height
    instantiate_staking(
        &mut app,
        staking_id,
        InstantiateMsg {
            token_info: TokenInfo::Existing {
                denom: DENOM.to_string(),
            },
            unstaking_duration: Some(Duration::Time(0)),
            active_threshold: Some(ActiveThreshold::AbsoluteCount {
                count: Uint128::new(1),
            }),
        },
    );
}

#[test]
#[should_panic(expected = "Must send reserve token 'ujuno'")]
fn test_stake_invalid_denom() {
    let mut app = mock_app();

    let staking_id = app.store_code(dao_voting_token_staked_contract());
    let addr = instantiate_staking(
        &mut app,
        staking_id,
        InstantiateMsg {
            token_info: TokenInfo::Existing {
                denom: DENOM.to_string(),
            },
            unstaking_duration: Some(Duration::Height(5)),
            active_threshold: None,
        },
    );

    // Try and stake an invalid denom
    stake_tokens(&mut app, addr, ADDR1, 100, INVALID_DENOM).unwrap();
}

#[test]
fn test_stake_valid_denom() {
    let mut app = mock_app();

    let staking_id = app.store_code(dao_voting_token_staked_contract());
    let addr = instantiate_staking(
        &mut app,
        staking_id,
        InstantiateMsg {
            token_info: TokenInfo::Existing {
                denom: DENOM.to_string(),
            },
            unstaking_duration: Some(Duration::Height(5)),
            active_threshold: None,
        },
    );

    // Try and stake an valid denom
    stake_tokens(&mut app, addr, ADDR1, 100, DENOM).unwrap();
    app.update_block(next_block);
}

#[test]
#[should_panic(expected = "Can only unstake less than or equal to the amount you have staked")]
fn test_unstake_none_staked() {
    let mut app = mock_app();

    let staking_id = app.store_code(dao_voting_token_staked_contract());
    let addr = instantiate_staking(
        &mut app,
        staking_id,
        InstantiateMsg {
            token_info: TokenInfo::Existing {
                denom: DENOM.to_string(),
            },
            unstaking_duration: Some(Duration::Height(5)),
            active_threshold: None,
        },
    );

    unstake_tokens(&mut app, addr, ADDR1, 100).unwrap();
}

#[test]
#[should_panic(expected = "Amount being unstaked must be non-zero")]
fn test_unstake_zero_tokens() {
    let mut app = mock_app();

    let staking_id = app.store_code(dao_voting_token_staked_contract());
    let addr = instantiate_staking(
        &mut app,
        staking_id,
        InstantiateMsg {
            token_info: TokenInfo::Existing {
                denom: DENOM.to_string(),
            },
            unstaking_duration: Some(Duration::Height(5)),
            active_threshold: None,
        },
    );

    unstake_tokens(&mut app, addr, ADDR1, 0).unwrap();
}

#[test]
#[should_panic(expected = "Can only unstake less than or equal to the amount you have staked")]
fn test_unstake_invalid_balance() {
    let mut app = mock_app();

    let staking_id = app.store_code(dao_voting_token_staked_contract());
    let addr = instantiate_staking(
        &mut app,
        staking_id,
        InstantiateMsg {
            token_info: TokenInfo::Existing {
                denom: DENOM.to_string(),
            },
            unstaking_duration: Some(Duration::Height(5)),
            active_threshold: None,
        },
    );

    // Stake some tokens
    stake_tokens(&mut app, addr.clone(), ADDR1, 100, DENOM).unwrap();
    app.update_block(next_block);

    // Try and unstake too many
    unstake_tokens(&mut app, addr, ADDR1, 200).unwrap();
}

#[test]
fn test_unstake() {
    let mut app = mock_app();

    let staking_id = app.store_code(dao_voting_token_staked_contract());
    let addr = instantiate_staking(
        &mut app,
        staking_id,
        InstantiateMsg {
            token_info: TokenInfo::Existing {
                denom: DENOM.to_string(),
            },
            unstaking_duration: Some(Duration::Height(5)),
            active_threshold: None,
        },
    );

    // Stake some tokens
    stake_tokens(&mut app, addr.clone(), ADDR1, 100, DENOM).unwrap();
    app.update_block(next_block);

    // Unstake some
    unstake_tokens(&mut app, addr.clone(), ADDR1, 75).unwrap();

    // Query claims
    let claims = get_claims(&mut app, addr.clone(), ADDR1.to_string());
    assert_eq!(claims.claims.len(), 1);
    app.update_block(next_block);

    // Unstake the rest
    unstake_tokens(&mut app, addr.clone(), ADDR1, 25).unwrap();

    // Query claims
    let claims = get_claims(&mut app, addr, ADDR1.to_string());
    assert_eq!(claims.claims.len(), 2);
}

#[test]
fn test_unstake_no_unstaking_duration() {
    let mut app = mock_app();

    let staking_id = app.store_code(dao_voting_token_staked_contract());
    let addr = instantiate_staking(
        &mut app,
        staking_id,
        InstantiateMsg {
            token_info: TokenInfo::Existing {
                denom: DENOM.to_string(),
            },
            unstaking_duration: None,
            active_threshold: None,
        },
    );

    // Stake some tokens
    stake_tokens(&mut app, addr.clone(), ADDR1, 100, DENOM).unwrap();
    app.update_block(next_block);

    // Unstake some tokens
    unstake_tokens(&mut app, addr.clone(), ADDR1, 75).unwrap();

    app.update_block(next_block);

    let balance = get_balance(&mut app, ADDR1, DENOM);
    // 10000 (initial bal) - 100 (staked) + 75 (unstaked) = 9975
    assert_eq!(balance, Uint128::new(9975));

    // Unstake the rest
    unstake_tokens(&mut app, addr, ADDR1, 25).unwrap();

    let balance = get_balance(&mut app, ADDR1, DENOM);
    // 10000 (initial bal) - 100 (staked) + 75 (unstaked 1) + 25 (unstaked 2) = 10000
    assert_eq!(balance, Uint128::new(10000))
}

#[test]
#[should_panic(expected = "Nothing to claim")]
fn test_claim_no_claims() {
    let mut app = mock_app();

    let staking_id = app.store_code(dao_voting_token_staked_contract());
    let addr = instantiate_staking(
        &mut app,
        staking_id,
        InstantiateMsg {
            token_info: TokenInfo::Existing {
                denom: DENOM.to_string(),
            },
            unstaking_duration: Some(Duration::Height(5)),
            active_threshold: None,
        },
    );

    claim(&mut app, addr, ADDR1).unwrap();
}

#[test]
#[should_panic(expected = "Nothing to claim")]
fn test_claim_claim_not_reached() {
    let mut app = mock_app();

    let staking_id = app.store_code(dao_voting_token_staked_contract());
    let addr = instantiate_staking(
        &mut app,
        staking_id,
        InstantiateMsg {
            token_info: TokenInfo::Existing {
                denom: DENOM.to_string(),
            },
            unstaking_duration: Some(Duration::Height(5)),
            active_threshold: None,
        },
    );

    // Stake some tokens
    stake_tokens(&mut app, addr.clone(), ADDR1, 100, DENOM).unwrap();
    app.update_block(next_block);

    // Unstake them to create the claims
    unstake_tokens(&mut app, addr.clone(), ADDR1, 100).unwrap();
    app.update_block(next_block);

    // We have a claim but it isnt reached yet so this will still fail
    claim(&mut app, addr, ADDR1).unwrap();
}

#[test]
fn test_claim() {
    let mut app = mock_app();

    let staking_id = app.store_code(dao_voting_token_staked_contract());
    let addr = instantiate_staking(
        &mut app,
        staking_id,
        InstantiateMsg {
            token_info: TokenInfo::Existing {
                denom: DENOM.to_string(),
            },
            unstaking_duration: Some(Duration::Height(5)),
            active_threshold: None,
        },
    );

    // Stake some tokens
    stake_tokens(&mut app, addr.clone(), ADDR1, 100, DENOM).unwrap();
    app.update_block(next_block);

    // Unstake some to create the claims
    unstake_tokens(&mut app, addr.clone(), ADDR1, 75).unwrap();
    app.update_block(|b| {
        b.height += 5;
        b.time = b.time.plus_seconds(25);
    });

    // Claim
    claim(&mut app, addr.clone(), ADDR1).unwrap();

    // Query balance
    let balance = get_balance(&mut app, ADDR1, DENOM);
    // 10000 (initial bal) - 100 (staked) + 75 (unstaked) = 9975
    assert_eq!(balance, Uint128::new(9975));

    // Unstake the rest
    unstake_tokens(&mut app, addr.clone(), ADDR1, 25).unwrap();
    app.update_block(|b| {
        b.height += 10;
        b.time = b.time.plus_seconds(50);
    });

    // Claim
    claim(&mut app, addr, ADDR1).unwrap();

    // Query balance
    let balance = get_balance(&mut app, ADDR1, DENOM);
    // 10000 (initial bal) - 100 (staked) + 75 (unstaked 1) + 25 (unstaked 2) = 10000
    assert_eq!(balance, Uint128::new(10000));
}

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_update_config_invalid_sender() {
    let mut app = mock_app();

    let staking_id = app.store_code(dao_voting_token_staked_contract());
    let addr = instantiate_staking(
        &mut app,
        staking_id,
        InstantiateMsg {
            token_info: TokenInfo::Existing {
                denom: DENOM.to_string(),
            },
            unstaking_duration: Some(Duration::Height(5)),
            active_threshold: None,
        },
    );

    // From ADDR2, so not owner or manager
    update_config(&mut app, addr, ADDR2, Some(Duration::Height(10))).unwrap();
}

#[test]
fn test_update_config_as_owner() {
    let mut app = mock_app();

    let staking_id = app.store_code(dao_voting_token_staked_contract());
    let addr = instantiate_staking(
        &mut app,
        staking_id,
        InstantiateMsg {
            token_info: TokenInfo::Existing {
                denom: DENOM.to_string(),
            },
            unstaking_duration: Some(Duration::Height(5)),
            active_threshold: None,
        },
    );

    // Swap owner and manager, change duration
    update_config(&mut app, addr.clone(), DAO_ADDR, Some(Duration::Height(10))).unwrap();

    let config = get_config(&mut app, addr);
    assert_eq!(
        Config {
            unstaking_duration: Some(Duration::Height(10)),
        },
        config
    );
}

#[test]
#[should_panic(expected = "Invalid unstaking duration, unstaking duration cannot be 0")]
fn test_update_config_invalid_duration() {
    let mut app = mock_app();

    let staking_id = app.store_code(dao_voting_token_staked_contract());
    let addr = instantiate_staking(
        &mut app,
        staking_id,
        InstantiateMsg {
            token_info: TokenInfo::Existing {
                denom: DENOM.to_string(),
            },
            unstaking_duration: Some(Duration::Height(5)),
            active_threshold: None,
        },
    );

    // Change duration and manager as manager cannot change owner
    update_config(&mut app, addr, DAO_ADDR, Some(Duration::Height(0))).unwrap();
}

#[test]
fn test_query_dao() {
    let mut app = mock_app();

    let staking_id = app.store_code(dao_voting_token_staked_contract());
    let addr = instantiate_staking(
        &mut app,
        staking_id,
        InstantiateMsg {
            token_info: TokenInfo::Existing {
                denom: DENOM.to_string(),
            },
            unstaking_duration: Some(Duration::Height(5)),
            active_threshold: None,
        },
    );

    let msg = QueryMsg::Dao {};
    let dao: Addr = app.wrap().query_wasm_smart(addr, &msg).unwrap();
    assert_eq!(dao, Addr::unchecked(DAO_ADDR));
}

#[test]
fn test_query_info() {
    let mut app = mock_app();

    let staking_id = app.store_code(dao_voting_token_staked_contract());
    let addr = instantiate_staking(
        &mut app,
        staking_id,
        InstantiateMsg {
            token_info: TokenInfo::Existing {
                denom: DENOM.to_string(),
            },
            unstaking_duration: Some(Duration::Height(5)),
            active_threshold: None,
        },
    );

    let msg = QueryMsg::Info {};
    let resp: InfoResponse = app.wrap().query_wasm_smart(addr, &msg).unwrap();
    assert_eq!(resp.info.contract, "crates.io:dao-voting-token-staked");
}

#[test]
fn test_query_claims() {
    let mut app = mock_app();

    let staking_id = app.store_code(dao_voting_token_staked_contract());
    let addr = instantiate_staking(
        &mut app,
        staking_id,
        InstantiateMsg {
            token_info: TokenInfo::Existing {
                denom: DENOM.to_string(),
            },
            unstaking_duration: Some(Duration::Height(5)),
            active_threshold: None,
        },
    );

    let claims = get_claims(&mut app, addr.clone(), ADDR1.to_string());
    assert_eq!(claims.claims.len(), 0);

    // Stake some tokens
    stake_tokens(&mut app, addr.clone(), ADDR1, 100, DENOM).unwrap();
    app.update_block(next_block);

    // Unstake some tokens
    unstake_tokens(&mut app, addr.clone(), ADDR1, 25).unwrap();
    app.update_block(next_block);

    let claims = get_claims(&mut app, addr.clone(), ADDR1.to_string());
    assert_eq!(claims.claims.len(), 1);

    unstake_tokens(&mut app, addr.clone(), ADDR1, 25).unwrap();
    app.update_block(next_block);

    let claims = get_claims(&mut app, addr, ADDR1.to_string());
    assert_eq!(claims.claims.len(), 2);
}

#[test]
fn test_query_get_config() {
    let mut app = mock_app();

    let staking_id = app.store_code(dao_voting_token_staked_contract());
    let addr = instantiate_staking(
        &mut app,
        staking_id,
        InstantiateMsg {
            token_info: TokenInfo::Existing {
                denom: DENOM.to_string(),
            },
            unstaking_duration: Some(Duration::Height(5)),
            active_threshold: None,
        },
    );

    let config = get_config(&mut app, addr);
    assert_eq!(
        config,
        Config {
            unstaking_duration: Some(Duration::Height(5)),
        }
    )
}

#[test]
fn test_voting_power_queries() {
    let mut app = mock_app();

    let staking_id = app.store_code(dao_voting_token_staked_contract());
    let addr = instantiate_staking(
        &mut app,
        staking_id,
        InstantiateMsg {
            token_info: TokenInfo::Existing {
                denom: DENOM.to_string(),
            },
            unstaking_duration: Some(Duration::Height(5)),
            active_threshold: None,
        },
    );

    // Total power is 0
    let resp = get_total_power_at_height(&mut app, addr.clone(), None);
    assert!(resp.power.is_zero());

    // ADDR1 has no power, none staked
    let resp = get_voting_power_at_height(&mut app, addr.clone(), ADDR1.to_string(), None);
    assert!(resp.power.is_zero());

    // ADDR1 stakes
    stake_tokens(&mut app, addr.clone(), ADDR1, 100, DENOM).unwrap();
    app.update_block(next_block);

    // Total power is 100
    let resp = get_total_power_at_height(&mut app, addr.clone(), None);
    assert_eq!(resp.power, Uint128::new(100));

    // ADDR1 has 100 power
    let resp = get_voting_power_at_height(&mut app, addr.clone(), ADDR1.to_string(), None);
    assert_eq!(resp.power, Uint128::new(100));

    // ADDR2 still has 0 power
    let resp = get_voting_power_at_height(&mut app, addr.clone(), ADDR2.to_string(), None);
    assert!(resp.power.is_zero());

    // ADDR2 stakes
    stake_tokens(&mut app, addr.clone(), ADDR2, 50, DENOM).unwrap();
    app.update_block(next_block);
    let prev_height = app.block_info().height - 1;

    // Query the previous height, total 100, ADDR1 100, ADDR2 0
    // Total power is 100
    let resp = get_total_power_at_height(&mut app, addr.clone(), Some(prev_height));
    assert_eq!(resp.power, Uint128::new(100));

    // ADDR1 has 100 power
    let resp =
        get_voting_power_at_height(&mut app, addr.clone(), ADDR1.to_string(), Some(prev_height));
    assert_eq!(resp.power, Uint128::new(100));

    // ADDR2 still has 0 power
    let resp =
        get_voting_power_at_height(&mut app, addr.clone(), ADDR2.to_string(), Some(prev_height));
    assert!(resp.power.is_zero());

    // For current height, total 150, ADDR1 100, ADDR2 50
    // Total power is 150
    let resp = get_total_power_at_height(&mut app, addr.clone(), None);
    assert_eq!(resp.power, Uint128::new(150));

    // ADDR1 has 100 power
    let resp = get_voting_power_at_height(&mut app, addr.clone(), ADDR1.to_string(), None);
    assert_eq!(resp.power, Uint128::new(100));

    // ADDR2 now has 50 power
    let resp = get_voting_power_at_height(&mut app, addr.clone(), ADDR2.to_string(), None);
    assert_eq!(resp.power, Uint128::new(50));

    // ADDR1 unstakes half
    unstake_tokens(&mut app, addr.clone(), ADDR1, 50).unwrap();
    app.update_block(next_block);
    let prev_height = app.block_info().height - 1;

    // Query the previous height, total 150, ADDR1 100, ADDR2 50
    // Total power is 100
    let resp = get_total_power_at_height(&mut app, addr.clone(), Some(prev_height));
    assert_eq!(resp.power, Uint128::new(150));

    // ADDR1 has 100 power
    let resp =
        get_voting_power_at_height(&mut app, addr.clone(), ADDR1.to_string(), Some(prev_height));
    assert_eq!(resp.power, Uint128::new(100));

    // ADDR2 still has 0 power
    let resp =
        get_voting_power_at_height(&mut app, addr.clone(), ADDR2.to_string(), Some(prev_height));
    assert_eq!(resp.power, Uint128::new(50));

    // For current height, total 100, ADDR1 50, ADDR2 50
    // Total power is 100
    let resp = get_total_power_at_height(&mut app, addr.clone(), None);
    assert_eq!(resp.power, Uint128::new(100));

    // ADDR1 has 50 power
    let resp = get_voting_power_at_height(&mut app, addr.clone(), ADDR1.to_string(), None);
    assert_eq!(resp.power, Uint128::new(50));

    // ADDR2 now has 50 power
    let resp = get_voting_power_at_height(&mut app, addr, ADDR2.to_string(), None);
    assert_eq!(resp.power, Uint128::new(50));
}

#[test]
fn test_query_list_stakers() {
    let mut app = mock_app();

    let staking_id = app.store_code(dao_voting_token_staked_contract());
    let addr = instantiate_staking(
        &mut app,
        staking_id,
        InstantiateMsg {
            token_info: TokenInfo::Existing {
                denom: DENOM.to_string(),
            },
            unstaking_duration: Some(Duration::Height(5)),
            active_threshold: None,
        },
    );

    // ADDR1 stakes
    stake_tokens(&mut app, addr.clone(), ADDR1, 100, DENOM).unwrap();

    // ADDR2 stakes
    stake_tokens(&mut app, addr.clone(), ADDR2, 50, DENOM).unwrap();

    // check entire result set
    let stakers: ListStakersResponse = app
        .wrap()
        .query_wasm_smart(
            addr.clone(),
            &QueryMsg::ListStakers {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();

    let test_res = ListStakersResponse {
        stakers: vec![
            StakerBalanceResponse {
                address: ADDR1.to_string(),
                balance: Uint128::new(100),
            },
            StakerBalanceResponse {
                address: ADDR2.to_string(),
                balance: Uint128::new(50),
            },
        ],
    };

    assert_eq!(stakers, test_res);

    // skipped 1, check result
    let stakers: ListStakersResponse = app
        .wrap()
        .query_wasm_smart(
            addr.clone(),
            &QueryMsg::ListStakers {
                start_after: Some(ADDR1.to_string()),
                limit: None,
            },
        )
        .unwrap();

    let test_res = ListStakersResponse {
        stakers: vec![StakerBalanceResponse {
            address: ADDR2.to_string(),
            balance: Uint128::new(50),
        }],
    };

    assert_eq!(stakers, test_res);

    // skipped 2, check result. should be nothing
    let stakers: ListStakersResponse = app
        .wrap()
        .query_wasm_smart(
            addr,
            &QueryMsg::ListStakers {
                start_after: Some(ADDR2.to_string()),
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(stakers, ListStakersResponse { stakers: vec![] });
}

#[test]
#[should_panic(expected = "Active threshold count must be greater than zero")]
fn test_instantiate_zero_active_threshold_count() {
    let mut app = mock_app();

    let staking_id = app.store_code(dao_voting_token_staked_contract());
    instantiate_staking(
        &mut app,
        staking_id,
        InstantiateMsg {
            token_info: TokenInfo::Existing {
                denom: DENOM.to_string(),
            },
            unstaking_duration: Some(Duration::Height(5)),
            active_threshold: Some(ActiveThreshold::AbsoluteCount {
                count: Uint128::zero(),
            }),
        },
    );
}

#[test]
fn test_active_threshold_absolute_count() {
    let mut app = mock_app();

    let staking_id = app.store_code(dao_voting_token_staked_contract());

    let addr = instantiate_staking(
        &mut app,
        staking_id,
        InstantiateMsg {
            token_info: TokenInfo::Existing {
                denom: DENOM.to_string(),
            },
            unstaking_duration: Some(Duration::Height(5)),
            active_threshold: Some(ActiveThreshold::AbsoluteCount {
                count: Uint128::new(100),
            }),
        },
    );

    // Not active as none staked
    let is_active: IsActiveResponse = app
        .wrap()
        .query_wasm_smart(addr.clone(), &QueryMsg::IsActive {})
        .unwrap();
    assert!(!is_active.active);

    // Stake 100 tokens
    stake_tokens(&mut app, addr.clone(), ADDR1, 100, DENOM).unwrap();
    app.update_block(next_block);

    // Active as enough staked
    let is_active: IsActiveResponse = app
        .wrap()
        .query_wasm_smart(addr, &QueryMsg::IsActive {})
        .unwrap();
    assert!(is_active.active);
}

#[test]
fn test_active_threshold_percent() {
    let mut app = mock_app();

    let staking_id = app.store_code(dao_voting_token_staked_contract());
    let addr = instantiate_staking(
        &mut app,
        staking_id,
        InstantiateMsg {
            token_info: TokenInfo::Existing {
                denom: DENOM.to_string(),
            },
            unstaking_duration: Some(Duration::Height(5)),
            active_threshold: Some(ActiveThreshold::Percentage {
                percent: Decimal::percent(20),
            }),
        },
    );

    // Not active as none staked
    let is_active: IsActiveResponse = app
        .wrap()
        .query_wasm_smart(addr.clone(), &QueryMsg::IsActive {})
        .unwrap();
    assert!(!is_active.active);

    // Stake 6000 tokens, now active
    stake_tokens(&mut app, addr.clone(), ADDR1, 6000, DENOM).unwrap();
    app.update_block(next_block);

    // Active as enough staked
    let is_active: IsActiveResponse = app
        .wrap()
        .query_wasm_smart(addr, &QueryMsg::IsActive {})
        .unwrap();
    assert!(is_active.active);
}

#[test]
fn test_active_threshold_percent_rounds_up() {
    let mut app = mock_app();

    let staking_id = app.store_code(dao_voting_token_staked_contract());
    let addr = instantiate_staking(
        &mut app,
        staking_id,
        InstantiateMsg {
            token_info: TokenInfo::Existing {
                denom: ODD_DENOM.to_string(),
            },
            unstaking_duration: Some(Duration::Height(5)),
            active_threshold: Some(ActiveThreshold::Percentage {
                percent: Decimal::percent(50),
            }),
        },
    );

    // Not active as none staked
    let is_active: IsActiveResponse = app
        .wrap()
        .query_wasm_smart(addr.clone(), &QueryMsg::IsActive {})
        .unwrap();
    assert!(!is_active.active);

    // Stake 2 tokens, should not be active.
    stake_tokens(&mut app, addr.clone(), ADDR1, 2, ODD_DENOM).unwrap();
    app.update_block(next_block);

    let is_active: IsActiveResponse = app
        .wrap()
        .query_wasm_smart(addr.clone(), &QueryMsg::IsActive {})
        .unwrap();
    assert!(!is_active.active);

    // Stake 1 more token, should now be active.
    stake_tokens(&mut app, addr.clone(), ADDR1, 1, ODD_DENOM).unwrap();
    app.update_block(next_block);

    let is_active: IsActiveResponse = app
        .wrap()
        .query_wasm_smart(addr, &QueryMsg::IsActive {})
        .unwrap();
    assert!(is_active.active);
}

#[test]
fn test_active_threshold_none() {
    let mut app = App::default();

    let staking_id = app.store_code(dao_voting_token_staked_contract());
    let addr = instantiate_staking(
        &mut app,
        staking_id,
        InstantiateMsg {
            token_info: TokenInfo::Existing {
                denom: DENOM.to_string(),
            },
            unstaking_duration: Some(Duration::Height(5)),
            active_threshold: None,
        },
    );

    // Active as no threshold
    let is_active: IsActiveResponse = app
        .wrap()
        .query_wasm_smart(addr, &QueryMsg::IsActive {})
        .unwrap();
    assert!(is_active.active);
}

#[test]
fn test_update_active_threshold() {
    let mut app = mock_app();

    let staking_id = app.store_code(dao_voting_token_staked_contract());
    let addr = instantiate_staking(
        &mut app,
        staking_id,
        InstantiateMsg {
            token_info: TokenInfo::Existing {
                denom: DENOM.to_string(),
            },
            unstaking_duration: Some(Duration::Height(5)),
            active_threshold: None,
        },
    );

    let resp: ActiveThresholdResponse = app
        .wrap()
        .query_wasm_smart(addr.clone(), &QueryMsg::ActiveThreshold {})
        .unwrap();
    assert_eq!(resp.active_threshold, None);

    let msg = ExecuteMsg::UpdateActiveThreshold {
        new_threshold: Some(ActiveThreshold::AbsoluteCount {
            count: Uint128::new(100),
        }),
    };

    // Expect failure as sender is not the DAO
    app.execute_contract(Addr::unchecked(ADDR1), addr.clone(), &msg, &[])
        .unwrap_err();

    // Expect success as sender is the DAO
    app.execute_contract(Addr::unchecked(DAO_ADDR), addr.clone(), &msg, &[])
        .unwrap();

    let resp: ActiveThresholdResponse = app
        .wrap()
        .query_wasm_smart(addr, &QueryMsg::ActiveThreshold {})
        .unwrap();
    assert_eq!(
        resp.active_threshold,
        Some(ActiveThreshold::AbsoluteCount {
            count: Uint128::new(100)
        })
    );
}

#[test]
#[should_panic(
    expected = "Active threshold percentage must be greater than 0 and not greater than 1"
)]
fn test_active_threshold_percentage_gt_100() {
    let mut app = App::default();

    let staking_id = app.store_code(dao_voting_token_staked_contract());
    instantiate_staking(
        &mut app,
        staking_id,
        InstantiateMsg {
            token_info: TokenInfo::Existing {
                denom: DENOM.to_string(),
            },
            unstaking_duration: Some(Duration::Height(5)),
            active_threshold: Some(ActiveThreshold::Percentage {
                percent: Decimal::percent(120),
            }),
        },
    );
}

#[test]
#[should_panic(
    expected = "Active threshold percentage must be greater than 0 and not greater than 1"
)]
fn test_active_threshold_percentage_lte_0() {
    let mut app = App::default();

    let staking_id = app.store_code(dao_voting_token_staked_contract());
    instantiate_staking(
        &mut app,
        staking_id,
        InstantiateMsg {
            token_info: TokenInfo::Existing {
                denom: DENOM.to_string(),
            },
            unstaking_duration: Some(Duration::Height(5)),
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

    let staking_id = app.store_code(dao_voting_token_staked_contract());
    instantiate_staking(
        &mut app,
        staking_id,
        InstantiateMsg {
            token_info: TokenInfo::Existing {
                denom: DENOM.to_string(),
            },
            unstaking_duration: Some(Duration::Height(5)),
            active_threshold: Some(ActiveThreshold::AbsoluteCount {
                count: Uint128::new(30001),
            }),
        },
    );
}

#[test]
fn test_add_remove_hooks() {
    let mut app = App::default();

    let staking_id = app.store_code(dao_voting_token_staked_contract());

    let addr = instantiate_staking(
        &mut app,
        staking_id,
        InstantiateMsg {
            token_info: TokenInfo::Existing {
                denom: DENOM.to_string(),
            },
            unstaking_duration: Some(Duration::Height(5)),
            active_threshold: None,
        },
    );

    // No hooks exist.
    let resp: GetHooksResponse = app
        .wrap()
        .query_wasm_smart(addr.clone(), &QueryMsg::GetHooks {})
        .unwrap();
    assert_eq!(resp.hooks, Vec::<String>::new());

    // Add a hook.
    app.execute_contract(
        Addr::unchecked(DAO_ADDR),
        addr.clone(),
        &ExecuteMsg::AddHook {
            addr: "hook".to_string(),
        },
        &[],
    )
    .unwrap();

    // One hook exists.
    let resp: GetHooksResponse = app
        .wrap()
        .query_wasm_smart(addr.clone(), &QueryMsg::GetHooks {})
        .unwrap();
    assert_eq!(resp.hooks, vec!["hook".to_string()]);

    // Remove hook.
    app.execute_contract(
        Addr::unchecked(DAO_ADDR),
        addr.clone(),
        &ExecuteMsg::RemoveHook {
            addr: "hook".to_string(),
        },
        &[],
    )
    .unwrap();

    // No hook exists.
    let resp: GetHooksResponse = app
        .wrap()
        .query_wasm_smart(addr, &QueryMsg::GetHooks {})
        .unwrap();
    assert_eq!(resp.hooks, Vec::<String>::new());
}

#[test]
fn test_staking_hooks() {
    let mut app = mock_app();

    let staking_id = app.store_code(dao_voting_token_staked_contract());
    let hook_id = app.store_code(dao_proposal_hook_counter_contract());

    let hook = app
        .instantiate_contract(
            hook_id,
            Addr::unchecked(DAO_ADDR),
            &dao_proposal_hook_counter::msg::InstantiateMsg {
                should_error: false,
            },
            &[],
            "hook counter".to_string(),
            None,
        )
        .unwrap();

    let addr = instantiate_staking(
        &mut app,
        staking_id,
        InstantiateMsg {
            token_info: TokenInfo::Existing {
                denom: DENOM.to_string(),
            },
            unstaking_duration: Some(Duration::Height(5)),
            active_threshold: None,
        },
    );

    // Add a staking hook.
    app.execute_contract(
        Addr::unchecked(DAO_ADDR),
        addr.clone(),
        &ExecuteMsg::AddHook {
            addr: hook.to_string(),
        },
        &[],
    )
    .unwrap();

    // Stake some tokens
    let res = stake_tokens(&mut app, addr.clone(), ADDR1, 100, DENOM).unwrap();

    // Make sure hook is included in response
    assert_eq!("stake_hook", res.events.last().unwrap().attributes[1].value);

    app.update_block(next_block);

    // Unstake some
    let res = unstake_tokens(&mut app, addr, ADDR1, 75).unwrap();

    // Make sure hook is included in response
    assert_eq!("stake_hook", res.events.last().unwrap().attributes[1].value);
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
