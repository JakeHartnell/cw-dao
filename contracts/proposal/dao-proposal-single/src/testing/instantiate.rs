use cosmwasm_std::{to_json_binary, Addr, Coin, Decimal, Empty, Uint128};
use cw20::Cw20Coin;

use cw_multi_test::{next_block, App, BankSudo, Executor, SudoMsg};
use cw_utils::Duration;
use dao_interface::state::{Admin, ModuleInstantiateInfo};
use dao_pre_propose_single as cppbps;

use dao_testing::contracts::{
    cw20_base_contract, cw20_stake_contract, cw4_group_contract, cw721_base_contract,
    dao_dao_core_contract, dao_pre_propose_single_contract, dao_proposal_single_contract,
    dao_voting_cw20_staked_contract, dao_voting_cw4_contract, dao_voting_cw721_staked_contract,
    dao_voting_token_staked_contract,
};
use dao_voting::{
    deposit::{DepositRefundPolicy, UncheckedDepositInfo, VotingModuleTokenType},
    pre_propose::{PreProposeInfo, PreProposeSubmissionPolicy},
    threshold::{ActiveThreshold, PercentageThreshold, Threshold::ThresholdQuorum},
};
use dao_voting_cw4::msg::GroupContract;

use crate::msg::InstantiateMsg;

use super::CREATOR_ADDR;

pub(crate) fn get_pre_propose_info(
    app: &mut App,
    deposit_info: Option<UncheckedDepositInfo>,
    open_proposal_submission: bool,
) -> PreProposeInfo {
    let pre_propose_contract = app.store_code(dao_pre_propose_single_contract());

    let submission_policy = if open_proposal_submission {
        PreProposeSubmissionPolicy::Anyone { denylist: vec![] }
    } else {
        PreProposeSubmissionPolicy::Specific {
            dao_members: true,
            allowlist: vec![],
            denylist: vec![],
        }
    };

    PreProposeInfo::ModuleMayPropose {
        info: ModuleInstantiateInfo {
            code_id: pre_propose_contract,
            msg: to_json_binary(&cppbps::InstantiateMsg {
                deposit_info,
                submission_policy,
                extension: Empty::default(),
            })
            .unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "pre_propose_contract".to_string(),
        },
    }
}

pub(crate) fn get_default_token_dao_proposal_module_instantiate(app: &mut App) -> InstantiateMsg {
    InstantiateMsg {
        veto: None,
        threshold: ThresholdQuorum {
            quorum: PercentageThreshold::Percent(Decimal::percent(15)),
            threshold: PercentageThreshold::Majority {},
        },
        max_voting_period: Duration::Time(604800), // One week.
        min_voting_period: None,
        only_members_execute: true,
        allow_revoting: false,
        pre_propose_info: get_pre_propose_info(
            app,
            Some(UncheckedDepositInfo {
                denom: dao_voting::deposit::DepositToken::VotingModuleToken {
                    token_type: VotingModuleTokenType::Cw20,
                },
                amount: Uint128::new(10_000_000),
                refund_policy: DepositRefundPolicy::OnlyPassed,
            }),
            false,
        ),
        close_proposal_on_execution_failure: true,
    }
}

// Same as above but no proposal deposit.
pub(crate) fn get_default_non_token_dao_proposal_module_instantiate(
    app: &mut App,
) -> InstantiateMsg {
    InstantiateMsg {
        veto: None,
        threshold: ThresholdQuorum {
            threshold: PercentageThreshold::Percent(Decimal::percent(15)),
            quorum: PercentageThreshold::Majority {},
        },
        max_voting_period: Duration::Time(604800), // One week.
        min_voting_period: None,
        only_members_execute: true,
        allow_revoting: false,
        pre_propose_info: get_pre_propose_info(app, None, false),
        close_proposal_on_execution_failure: true,
    }
}

pub(crate) fn instantiate_with_staked_cw721_governance(
    app: &mut App,
    proposal_module_instantiate: InstantiateMsg,
    initial_balances: Option<Vec<Cw20Coin>>,
) -> Addr {
    let proposal_module_code_id = app.store_code(dao_proposal_single_contract());

    let initial_balances = initial_balances.unwrap_or_else(|| {
        vec![Cw20Coin {
            address: CREATOR_ADDR.to_string(),
            amount: Uint128::new(100_000_000),
        }]
    });

    let initial_balances: Vec<Cw20Coin> = {
        let mut already_seen = vec![];
        initial_balances
            .into_iter()
            .filter(|Cw20Coin { address, amount: _ }| {
                if already_seen.contains(address) {
                    false
                } else {
                    already_seen.push(address.clone());
                    true
                }
            })
            .collect()
    };

    let cw721_id = app.store_code(cw721_base_contract());
    let cw721_stake_id = app.store_code(dao_voting_cw721_staked_contract());
    let core_contract_id = app.store_code(dao_dao_core_contract());

    let nft_address = app
        .instantiate_contract(
            cw721_id,
            Addr::unchecked("ekez"),
            &cw721_base::msg::InstantiateMsg {
                minter: "ekez".to_string(),
                symbol: "token".to_string(),
                name: "ekez token best token".to_string(),
            },
            &[],
            "nft-staking",
            None,
        )
        .unwrap();

    let instantiate_core = dao_interface::msg::InstantiateMsg {
        admin: None,
        name: "DAO DAO".to_string(),
        description: "A DAO that builds DAOs".to_string(),
        dao_uri: None,
        image_url: None,
        automatically_add_cw20s: true,
        automatically_add_cw721s: false,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: cw721_stake_id,
            msg: to_json_binary(&dao_voting_cw721_staked::msg::InstantiateMsg {
                unstaking_duration: None,
                nft_contract: dao_voting_cw721_staked::msg::NftContract::Existing {
                    address: nft_address.to_string(),
                },
                active_threshold: None,
            })
            .unwrap(),
            admin: None,
            funds: vec![],
            label: "DAO DAO voting module".to_string(),
        },
        proposal_modules_instantiate_info: vec![ModuleInstantiateInfo {
            code_id: proposal_module_code_id,
            msg: to_json_binary(&proposal_module_instantiate).unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "DAO DAO governance module.".to_string(),
        }],
        initial_items: None,
    };

    let core_addr = app
        .instantiate_contract(
            core_contract_id,
            Addr::unchecked(CREATOR_ADDR),
            &instantiate_core,
            &[],
            "DAO DAO",
            None,
        )
        .unwrap();

    let core_state: dao_interface::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(
            core_addr.clone(),
            &dao_interface::msg::QueryMsg::DumpState {},
        )
        .unwrap();
    let staking_addr = core_state.voting_module;

    for Cw20Coin { address, amount } in initial_balances {
        for i in 0..amount.u128() {
            app.execute_contract(
                Addr::unchecked("ekez"),
                nft_address.clone(),
                &cw721_base::msg::ExecuteMsg::<Option<Empty>, Empty>::Mint {
                    token_id: format!("{address}_{i}"),
                    owner: address.clone(),
                    token_uri: None,
                    extension: None,
                },
                &[],
            )
            .unwrap();
            app.execute_contract(
                Addr::unchecked(address.clone()),
                nft_address.clone(),
                &cw721_base::msg::ExecuteMsg::SendNft::<Option<Empty>, Empty> {
                    contract: staking_addr.to_string(),
                    token_id: format!("{address}_{i}"),
                    msg: to_json_binary("").unwrap(),
                },
                &[],
            )
            .unwrap();
        }
    }

    // Update the block so that staked balances appear.
    app.update_block(|block| block.height += 1);

    core_addr
}

pub(crate) fn instantiate_with_native_staked_balances_governance(
    app: &mut App,
    proposal_module_instantiate: InstantiateMsg,
    initial_balances: Option<Vec<Cw20Coin>>,
) -> Addr {
    let proposal_module_code_id = app.store_code(dao_proposal_single_contract());

    let initial_balances = initial_balances.unwrap_or_else(|| {
        vec![Cw20Coin {
            address: CREATOR_ADDR.to_string(),
            amount: Uint128::new(100_000_000),
        }]
    });

    // Collapse balances so that we can test double votes.
    let initial_balances: Vec<Cw20Coin> = {
        let mut already_seen = vec![];
        initial_balances
            .into_iter()
            .filter(|Cw20Coin { address, amount: _ }| {
                if already_seen.contains(address) {
                    false
                } else {
                    already_seen.push(address.clone());
                    true
                }
            })
            .collect()
    };

    let native_stake_id = app.store_code(dao_voting_token_staked_contract());
    let core_contract_id = app.store_code(dao_dao_core_contract());

    let instantiate_core = dao_interface::msg::InstantiateMsg {
        admin: None,
        name: "DAO DAO".to_string(),
        description: "A DAO that builds DAOs".to_string(),
        dao_uri: None,
        image_url: None,
        automatically_add_cw20s: true,
        automatically_add_cw721s: false,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: native_stake_id,
            msg: to_json_binary(&dao_voting_token_staked::msg::InstantiateMsg {
                token_info: dao_voting_token_staked::msg::TokenInfo::Existing {
                    denom: "ujuno".to_string(),
                },
                unstaking_duration: None,
                active_threshold: None,
            })
            .unwrap(),
            admin: None,
            funds: vec![],
            label: "DAO DAO voting module".to_string(),
        },
        proposal_modules_instantiate_info: vec![ModuleInstantiateInfo {
            code_id: proposal_module_code_id,
            msg: to_json_binary(&proposal_module_instantiate).unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "DAO DAO governance module.".to_string(),
        }],
        initial_items: None,
    };

    let core_addr = app
        .instantiate_contract(
            core_contract_id,
            Addr::unchecked(CREATOR_ADDR),
            &instantiate_core,
            &[],
            "DAO DAO",
            None,
        )
        .unwrap();

    let gov_state: dao_interface::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(
            core_addr.clone(),
            &dao_interface::msg::QueryMsg::DumpState {},
        )
        .unwrap();
    let native_staking_addr = gov_state.voting_module;

    for Cw20Coin { address, amount } in initial_balances {
        app.sudo(SudoMsg::Bank(BankSudo::Mint {
            to_address: address.clone(),
            amount: vec![Coin {
                denom: "ujuno".to_string(),
                amount,
            }],
        }))
        .unwrap();
        app.execute_contract(
            Addr::unchecked(&address),
            native_staking_addr.clone(),
            &dao_voting_token_staked::msg::ExecuteMsg::Stake {},
            &[Coin {
                amount,
                denom: "ujuno".to_string(),
            }],
        )
        .unwrap();
    }

    app.update_block(next_block);

    core_addr
}

pub(crate) fn instantiate_with_staked_balances_governance(
    app: &mut App,
    proposal_module_instantiate: InstantiateMsg,
    initial_balances: Option<Vec<Cw20Coin>>,
) -> Addr {
    let proposal_module_code_id = app.store_code(dao_proposal_single_contract());

    let initial_balances = initial_balances.unwrap_or_else(|| {
        vec![Cw20Coin {
            address: CREATOR_ADDR.to_string(),
            amount: Uint128::new(100_000_000),
        }]
    });

    // Collapse balances so that we can test double votes.
    let initial_balances: Vec<Cw20Coin> = {
        let mut already_seen = vec![];
        initial_balances
            .into_iter()
            .filter(|Cw20Coin { address, amount: _ }| {
                if already_seen.contains(address) {
                    false
                } else {
                    already_seen.push(address.clone());
                    true
                }
            })
            .collect()
    };

    let cw20_id = app.store_code(cw20_base_contract());
    let cw20_stake_id = app.store_code(cw20_stake_contract());
    let staked_balances_voting_id = app.store_code(dao_voting_cw20_staked_contract());
    let core_contract_id = app.store_code(dao_dao_core_contract());

    let instantiate_core = dao_interface::msg::InstantiateMsg {
        admin: None,
        name: "DAO DAO".to_string(),
        description: "A DAO that builds DAOs".to_string(),
        dao_uri: None,
        image_url: None,
        automatically_add_cw20s: true,
        automatically_add_cw721s: false,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: staked_balances_voting_id,
            msg: to_json_binary(&dao_voting_cw20_staked::msg::InstantiateMsg {
                active_threshold: None,
                token_info: dao_voting_cw20_staked::msg::TokenInfo::New {
                    code_id: cw20_id,
                    label: "DAO DAO governance token.".to_string(),
                    name: "DAO DAO".to_string(),
                    symbol: "DAO".to_string(),
                    decimals: 6,
                    initial_balances: initial_balances.clone(),
                    marketing: None,
                    staking_code_id: cw20_stake_id,
                    unstaking_duration: Some(Duration::Height(6)),
                    initial_dao_balance: None,
                },
            })
            .unwrap(),
            admin: None,
            funds: vec![],
            label: "DAO DAO voting module".to_string(),
        },
        proposal_modules_instantiate_info: vec![ModuleInstantiateInfo {
            code_id: proposal_module_code_id,
            msg: to_json_binary(&proposal_module_instantiate).unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "DAO DAO governance module.".to_string(),
        }],
        initial_items: None,
    };

    let core_addr = app
        .instantiate_contract(
            core_contract_id,
            Addr::unchecked(CREATOR_ADDR),
            &instantiate_core,
            &[],
            "DAO DAO",
            None,
        )
        .unwrap();

    let gov_state: dao_interface::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(
            core_addr.clone(),
            &dao_interface::msg::QueryMsg::DumpState {},
        )
        .unwrap();
    let voting_module = gov_state.voting_module;

    let staking_contract: Addr = app
        .wrap()
        .query_wasm_smart(
            voting_module.clone(),
            &dao_voting_cw20_staked::msg::QueryMsg::StakingContract {},
        )
        .unwrap();
    let token_contract: Addr = app
        .wrap()
        .query_wasm_smart(
            voting_module,
            &dao_interface::voting::Query::TokenContract {},
        )
        .unwrap();

    // Stake all the initial balances.
    for Cw20Coin { address, amount } in initial_balances {
        app.execute_contract(
            Addr::unchecked(address),
            token_contract.clone(),
            &cw20::Cw20ExecuteMsg::Send {
                contract: staking_contract.to_string(),
                amount,
                msg: to_json_binary(&cw20_stake::msg::ReceiveMsg::Stake {}).unwrap(),
            },
            &[],
        )
        .unwrap();
    }

    // Update the block so that those staked balances appear.
    app.update_block(|block| block.height += 1);

    core_addr
}

pub(crate) fn instantiate_with_staking_active_threshold(
    app: &mut App,
    proposal_module_instantiate: InstantiateMsg,
    initial_balances: Option<Vec<Cw20Coin>>,
    active_threshold: Option<ActiveThreshold>,
) -> Addr {
    let proposal_module_code_id = app.store_code(dao_proposal_single_contract());
    let cw20_id = app.store_code(cw20_base_contract());
    let cw20_staking_id = app.store_code(cw20_stake_contract());
    let core_id = app.store_code(dao_dao_core_contract());
    let votemod_id = app.store_code(dao_voting_cw20_staked_contract());

    let initial_balances = initial_balances.unwrap_or_else(|| {
        vec![Cw20Coin {
            address: CREATOR_ADDR.to_string(),
            amount: Uint128::new(100_000_000),
        }]
    });

    let governance_instantiate = dao_interface::msg::InstantiateMsg {
        admin: None,
        name: "DAO DAO".to_string(),
        description: "A DAO that builds DAOs".to_string(),
        dao_uri: None,
        image_url: None,
        automatically_add_cw20s: true,
        automatically_add_cw721s: true,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: votemod_id,
            msg: to_json_binary(&dao_voting_cw20_staked::msg::InstantiateMsg {
                token_info: dao_voting_cw20_staked::msg::TokenInfo::New {
                    code_id: cw20_id,
                    label: "DAO DAO governance token".to_string(),
                    name: "DAO".to_string(),
                    symbol: "DAO".to_string(),
                    decimals: 6,
                    initial_balances,
                    marketing: None,
                    staking_code_id: cw20_staking_id,
                    unstaking_duration: None,
                    initial_dao_balance: None,
                },
                active_threshold,
            })
            .unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "DAO DAO voting module".to_string(),
        },
        proposal_modules_instantiate_info: vec![ModuleInstantiateInfo {
            code_id: proposal_module_code_id,
            msg: to_json_binary(&proposal_module_instantiate).unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "DAO DAO governance module".to_string(),
        }],
        initial_items: None,
    };

    app.instantiate_contract(
        core_id,
        Addr::unchecked(CREATOR_ADDR),
        &governance_instantiate,
        &[],
        "DAO DAO",
        None,
    )
    .unwrap()
}

pub(crate) fn instantiate_with_cw4_groups_governance(
    app: &mut App,
    proposal_module_instantiate: InstantiateMsg,
    initial_weights: Option<Vec<Cw20Coin>>,
) -> Addr {
    let proposal_module_code_id = app.store_code(dao_proposal_single_contract());
    let cw4_id = app.store_code(cw4_group_contract());
    let core_id = app.store_code(dao_dao_core_contract());
    let votemod_id = app.store_code(dao_voting_cw4_contract());

    let initial_weights = initial_weights.unwrap_or_else(|| {
        vec![Cw20Coin {
            address: CREATOR_ADDR.to_string(),
            amount: Uint128::new(1),
        }]
    });

    // Remove duplicates so that we can test duplicate voting.
    let initial_weights: Vec<cw4::Member> = {
        let mut already_seen = vec![];
        initial_weights
            .into_iter()
            .filter(|Cw20Coin { address, .. }| {
                if already_seen.contains(address) {
                    false
                } else {
                    already_seen.push(address.clone());
                    true
                }
            })
            .map(|Cw20Coin { address, amount }| cw4::Member {
                addr: address,
                weight: amount.u128() as u64,
            })
            .collect()
    };

    let governance_instantiate = dao_interface::msg::InstantiateMsg {
        admin: None,
        name: "DAO DAO".to_string(),
        description: "A DAO that builds DAOs".to_string(),
        dao_uri: None,
        image_url: None,
        automatically_add_cw20s: true,
        automatically_add_cw721s: true,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: votemod_id,
            msg: to_json_binary(&dao_voting_cw4::msg::InstantiateMsg {
                group_contract: GroupContract::New {
                    cw4_group_code_id: cw4_id,
                    initial_members: initial_weights,
                },
            })
            .unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "DAO DAO voting module".to_string(),
        },
        proposal_modules_instantiate_info: vec![ModuleInstantiateInfo {
            code_id: proposal_module_code_id,
            msg: to_json_binary(&proposal_module_instantiate).unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "DAO DAO governance module".to_string(),
        }],
        initial_items: None,
    };

    let addr = app
        .instantiate_contract(
            core_id,
            Addr::unchecked(CREATOR_ADDR),
            &governance_instantiate,
            &[],
            "DAO DAO",
            None,
        )
        .unwrap();

    // Update the block so that weights appear.
    app.update_block(|block| block.height += 1);

    addr
}
