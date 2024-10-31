use cosmwasm_std::{
    to_json_binary, Addr, Binary, Deps, DepsMut, Empty, Env, MessageInfo, Response, StdResult,
    Uint128, WasmMsg,
};
use cw_multi_test::{next_block, App, Contract, ContractWrapper, Executor};
use dao_interface::query::SubDao;
use dao_testing::contracts::{
    cw20_base_contract, cw20_stake_contract, cw4_group_contract, dao_dao_core_contract,
    dao_proposal_single_contract, dao_voting_cw20_staked_contract, dao_voting_cw4_contract,
    v1::{
        cw20_stake_v1_contract, cw4_voting_v1_contract, cw_core_v1_contract,
        cw_proposal_single_v1_contract,
    },
};

use crate::{
    types::{V1CodeIds, V2CodeIds},
    ContractError,
};

pub(crate) const SENDER_ADDR: &str = "creator";

#[derive(Clone)]
pub struct CodeIds {
    pub core: u64,
    pub proposal_single: u64,
    pub cw20_base: u64,
    pub cw20_stake: u64,
    pub cw20_voting: u64,
    pub cw4_group: u64,
    pub cw4_voting: u64,
}

pub struct ExecuteParams {
    pub sub_daos: Option<Vec<SubDao>>,
    pub migrate_cw20: Option<bool>,
}

#[derive(Clone, Debug)]
pub struct ModuleAddrs {
    pub core: Addr,
    pub proposals: Vec<Addr>,
    pub voting: Addr,
    pub staking: Option<Addr>,
    pub token: Option<Addr>,
}

#[derive(Clone)]
pub enum VotingType {
    Cw4,
    Cw20,
    Cw20V03,
}

pub fn get_v1_code_ids(app: &mut App) -> (CodeIds, V1CodeIds) {
    let code_ids = CodeIds {
        core: app.store_code(cw_core_v1_contract()),
        proposal_single: app.store_code(cw_proposal_single_v1_contract()),
        cw20_base: app.store_code(cw20_base_contract()),
        cw20_stake: app.store_code(cw20_stake_v1_contract()),
        cw20_voting: app.store_code(dao_voting_cw20_staked_contract()),
        cw4_group: app.store_code(cw4_group_contract()),
        cw4_voting: app.store_code(cw4_voting_v1_contract()),
    };

    let v1_code_ids = V1CodeIds {
        proposal_single: code_ids.proposal_single,
        cw4_voting: code_ids.cw4_voting,
        cw20_stake: code_ids.cw20_stake,
        cw20_staked_balances_voting: code_ids.cw20_voting,
    };
    (code_ids, v1_code_ids)
}

pub fn get_v2_code_ids(app: &mut App) -> (CodeIds, V2CodeIds) {
    let code_ids = CodeIds {
        core: app.store_code(dao_dao_core_contract()),
        proposal_single: app.store_code(dao_proposal_single_contract()),
        cw20_base: app.store_code(cw20_base_contract()),
        cw20_stake: app.store_code(cw20_stake_contract()),
        cw20_voting: app.store_code(dao_voting_cw20_staked_contract()),
        cw4_group: app.store_code(cw4_group_contract()),
        cw4_voting: app.store_code(dao_voting_cw4_contract()),
    };

    let v2_code_ids = V2CodeIds {
        proposal_single: code_ids.proposal_single,
        cw4_voting: code_ids.cw4_voting,
        cw20_stake: code_ids.cw20_stake,
        cw20_staked_balances_voting: code_ids.cw20_voting,
    };
    (code_ids, v2_code_ids)
}

pub fn get_cw20_init_msg(code_ids: CodeIds) -> cw20_staked_balance_voting_v1::msg::InstantiateMsg {
    cw20_staked_balance_voting_v1::msg::InstantiateMsg {
        token_info: cw20_staked_balance_voting_v1::msg::TokenInfo::New {
            code_id: code_ids.cw20_base,
            label: "token".to_string(),
            name: "name".to_string(),
            symbol: "symbol".to_string(),
            decimals: 6,
            initial_balances: vec![cw20_v1::Cw20Coin {
                address: SENDER_ADDR.to_string(),
                amount: Uint128::new(2),
            }],
            marketing: None,
            staking_code_id: code_ids.cw20_stake,
            unstaking_duration: None,
            initial_dao_balance: Some(Uint128::new(100)),
        },
        active_threshold: None,
    }
}

pub fn get_cw4_init_msg(code_ids: CodeIds) -> cw4_voting_v1::msg::InstantiateMsg {
    cw4_voting_v1::msg::InstantiateMsg {
        cw4_group_code_id: code_ids.cw4_group,
        initial_members: vec![cw4_v1::Member {
            addr: SENDER_ADDR.to_string(),
            weight: 100,
        }],
    }
}

pub fn get_module_addrs(app: &mut App, core_addr: Addr) -> ModuleAddrs {
    // Get modules addrs
    let proposal_addrs: Vec<Addr> = {
        app.wrap()
            .query_wasm_smart(
                &core_addr,
                &cw_core_v1::msg::QueryMsg::ProposalModules {
                    start_at: None,
                    limit: None,
                },
            )
            .unwrap()
    };

    let voting_addr: Addr = app
        .wrap()
        .query_wasm_smart(&core_addr, &cw_core_v1::msg::QueryMsg::VotingModule {})
        .unwrap();

    let staking_addr: Option<Addr> = app
        .wrap()
        .query_wasm_smart(
            &voting_addr,
            &dao_voting_cw20_staked::msg::QueryMsg::StakingContract {},
        )
        .ok();

    let token_addr: Option<Addr> = app
        .wrap()
        .query_wasm_smart(
            &voting_addr,
            &dao_voting_cw20_staked::msg::QueryMsg::TokenContract {},
        )
        .ok();

    ModuleAddrs {
        core: core_addr,
        proposals: proposal_addrs,
        staking: staking_addr,
        voting: voting_addr,
        token: token_addr,
    }
}

pub fn set_dummy_proposal(app: &mut App, sender: Addr, core_addr: Addr, proposal_addr: Addr) {
    app.execute_contract(
        sender,
        proposal_addr,
        &cw_proposal_single_v1::msg::ExecuteMsg::Propose {
            title: "t".to_string(),
            description: "d".to_string(),
            msgs: vec![WasmMsg::Execute {
                contract_addr: core_addr.to_string(),
                msg: to_json_binary(&cw_core_v1::msg::ExecuteMsg::UpdateCw20List {
                    to_add: vec![],
                    to_remove: vec![],
                })
                .unwrap(),
                funds: vec![],
            }
            .into()],
        },
        &[],
    )
    .unwrap();
}

pub fn set_cw20_to_dao(app: &mut App, sender: Addr, addrs: ModuleAddrs) {
    let token_addr = addrs.token.unwrap();
    let staking_addr = addrs.staking.unwrap();

    // Stake tokens
    app.execute_contract(
        sender.clone(),
        token_addr.clone(),
        &cw20::Cw20ExecuteMsg::Send {
            contract: staking_addr.to_string(),
            amount: Uint128::new(1),
            msg: to_json_binary(&cw20_stake::msg::ReceiveMsg::Stake {}).unwrap(),
        },
        &[],
    )
    .unwrap();
    app.update_block(next_block);

    // ----
    // create a proposal and add tokens to the treasury.
    // ----

    app.execute_contract(
        sender.clone(),
        addrs.proposals[0].clone(),
        &cw_proposal_single_v1::msg::ExecuteMsg::Propose {
            title: "t".to_string(),
            description: "d".to_string(),
            msgs: vec![WasmMsg::Execute {
                contract_addr: addrs.core.to_string(),
                msg: to_json_binary(&cw_core_v1::msg::ExecuteMsg::UpdateCw20List {
                    to_add: vec![token_addr.to_string()],
                    to_remove: vec![],
                })
                .unwrap(),
                funds: vec![],
            }
            .into()],
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        sender.clone(),
        addrs.proposals[0].clone(),
        &cw_proposal_single_v1::msg::ExecuteMsg::Vote {
            proposal_id: 1,
            vote: voting_v1::Vote::Yes,
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        sender,
        addrs.proposals[0].clone(),
        &cw_proposal_single_v1::msg::ExecuteMsg::Execute { proposal_id: 1 },
        &[],
    )
    .unwrap();

    let tokens: Vec<cw_core_v1::query::Cw20BalanceResponse> = app
        .wrap()
        .query_wasm_smart(
            &addrs.core,
            &cw_core_v1::msg::QueryMsg::Cw20Balances {
                start_at: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(
        tokens,
        vec![cw_core_v1::query::Cw20BalanceResponse {
            addr: token_addr,
            balance: Uint128::new(100),
        }]
    );
}

fn some_init(
    _deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: (),
) -> Result<Response, ContractError> {
    Ok(Response::default())
}
fn some_execute(
    _deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: (),
) -> Result<Response, ContractError> {
    Ok(Response::default())
}
fn some_query(_deps: Deps, _env: Env, _msg: ()) -> StdResult<Binary> {
    Ok(Binary::default())
}

pub fn demo_contract() -> Box<dyn Contract<Empty>> {
    Box::new(ContractWrapper::new(some_execute, some_init, some_query))
}
