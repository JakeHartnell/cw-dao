use std::ops::Add;

use cosmwasm_std::{
    coins,
    testing::{mock_dependencies, mock_env},
    to_json_binary, Addr, Attribute, BankMsg, Binary, ContractInfoResponse, CosmosMsg, Decimal,
    Empty, Reply, StdError, SubMsgResult, Uint128, WasmMsg, WasmQuery,
};
use cw2::ContractVersion;
use cw20::Cw20Coin;
use cw_denom::CheckedDenom;
use cw_hooks::{HookError, HooksResponse};
use cw_multi_test::{next_block, App, Executor};
use cw_utils::Duration;
use dao_interface::{
    state::{Admin, ModuleInstantiateInfo},
    voting::InfoResponse,
};
use dao_testing::{
    contracts::{dao_pre_propose_single_contract, dao_proposal_single_contract},
    ShouldExecute, TestSingleChoiceVote,
};
use dao_voting::{
    deposit::{CheckedDepositInfo, UncheckedDepositInfo, VotingModuleTokenType},
    pre_propose::{PreProposeInfo, PreProposeSubmissionPolicy, ProposalCreationPolicy},
    proposal::{SingleChoiceProposeMsg as ProposeMsg, MAX_PROPOSAL_SIZE},
    reply::{
        failed_pre_propose_module_hook_id, mask_proposal_execution_proposal_id,
        mask_proposal_hook_index, mask_vote_hook_index,
    },
    status::Status,
    threshold::{ActiveThreshold, PercentageThreshold, Threshold},
    veto::{VetoConfig, VetoError},
    voting::{SingleChoiceAutoVote, Vote, Votes},
};

use crate::{
    contract::{migrate, CONTRACT_NAME, CONTRACT_VERSION},
    msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg},
    proposal::SingleChoiceProposal,
    query::{ProposalResponse, VoteInfo},
    state::Config,
    testing::{
        execute::{
            add_proposal_hook, add_proposal_hook_should_fail, add_vote_hook,
            add_vote_hook_should_fail, close_proposal, close_proposal_should_fail,
            execute_proposal, execute_proposal_should_fail, instantiate_cw20_base_default,
            make_proposal, mint_cw20s, mint_natives, remove_proposal_hook,
            remove_proposal_hook_should_fail, remove_vote_hook, remove_vote_hook_should_fail,
            update_rationale, vote_on_proposal, vote_on_proposal_should_fail,
        },
        instantiate::{
            get_default_non_token_dao_proposal_module_instantiate,
            get_default_token_dao_proposal_module_instantiate, get_pre_propose_info,
            instantiate_with_cw4_groups_governance, instantiate_with_staked_balances_governance,
            instantiate_with_staking_active_threshold,
        },
        queries::{
            query_balance_cw20, query_balance_native, query_creation_policy, query_dao_token,
            query_deposit_config_and_pre_propose_module, query_list_proposals,
            query_list_proposals_reverse, query_list_votes, query_pre_proposal_single_config,
            query_pre_proposal_single_deposit_info, query_proposal, query_proposal_config,
            query_proposal_hooks, query_single_proposal_module, query_vote_hooks,
            query_voting_module,
        },
    },
};
use dao_proposal_single::ContractError;

use super::{
    do_votes::do_votes_staked_balances,
    execute::vote_on_proposal_with_rationale,
    queries::{query_next_proposal_id, query_vote},
    CREATOR_ADDR,
};

struct CommonTest {
    app: App,
    core_addr: Addr,
    proposal_module: Addr,
    gov_token: Addr,
    proposal_id: u64,
}
fn setup_test(messages: Vec<CosmosMsg>) -> CommonTest {
    let mut app = App::default();
    let instantiate = get_default_token_dao_proposal_module_instantiate(&mut app);
    let core_addr = instantiate_with_staked_balances_governance(&mut app, instantiate, None);
    let proposal_module = query_single_proposal_module(&app, &core_addr);
    let gov_token = query_dao_token(&app, &core_addr);

    // Mint some tokens to pay the proposal deposit.
    mint_cw20s(&mut app, &gov_token, &core_addr, CREATOR_ADDR, 10_000_000);
    let proposal_id = make_proposal(&mut app, &proposal_module, CREATOR_ADDR, messages, None);

    CommonTest {
        app,
        core_addr,
        proposal_module,
        gov_token,
        proposal_id,
    }
}

#[test]
fn test_simple_propose_staked_balances() {
    let CommonTest {
        app,
        core_addr: _,
        proposal_module,
        gov_token,
        proposal_id,
    } = setup_test(vec![]);

    let created = query_proposal(&app, &proposal_module, proposal_id);
    let current_block = app.block_info();

    // These values just come from the default instantiate message
    // values.
    let expected = SingleChoiceProposal {
        title: "title".to_string(),
        description: "description".to_string(),
        proposer: Addr::unchecked(CREATOR_ADDR),
        start_height: current_block.height,
        expiration: Duration::Time(604800).after(&current_block),
        min_voting_period: None,
        threshold: Threshold::ThresholdQuorum {
            quorum: PercentageThreshold::Percent(Decimal::percent(15)),
            threshold: PercentageThreshold::Majority {},
        },
        allow_revoting: false,
        total_power: Uint128::new(100_000_000),
        msgs: vec![],
        status: Status::Open,
        veto: None,
        votes: Votes::zero(),
        individual_votes: Votes::zero(),
        delegation_module: None,
    };

    assert_eq!(created.proposal, expected);
    assert_eq!(created.id, 1u64);

    // Check that the deposit info for this proposal looks right.
    let (_, pre_propose) = query_deposit_config_and_pre_propose_module(&app, &proposal_module);
    let deposit_response = query_pre_proposal_single_deposit_info(&app, &pre_propose, proposal_id);

    assert_eq!(deposit_response.proposer, Addr::unchecked(CREATOR_ADDR));
    assert_eq!(
        deposit_response.deposit_info,
        Some(CheckedDepositInfo {
            denom: cw_denom::CheckedDenom::Cw20(gov_token),
            amount: Uint128::new(10_000_000),
            refund_policy: dao_voting::deposit::DepositRefundPolicy::OnlyPassed
        })
    );
}

#[test]
fn test_simple_proposal_cw4_voting() {
    let mut app = App::default();
    let instantiate = get_default_non_token_dao_proposal_module_instantiate(&mut app);
    let core_addr = instantiate_with_cw4_groups_governance(&mut app, instantiate, None);
    let proposal_module = query_single_proposal_module(&app, &core_addr);
    let id = make_proposal(&mut app, &proposal_module, CREATOR_ADDR, vec![], None);

    let created = query_proposal(&app, &proposal_module, id);
    let current_block = app.block_info();

    // These values just come from the default instantiate message
    // values.
    let expected = SingleChoiceProposal {
        title: "title".to_string(),
        description: "description".to_string(),
        proposer: Addr::unchecked(CREATOR_ADDR),
        start_height: current_block.height,
        expiration: Duration::Time(604800).after(&current_block),
        min_voting_period: None,
        threshold: Threshold::ThresholdQuorum {
            threshold: PercentageThreshold::Percent(Decimal::percent(15)),
            quorum: PercentageThreshold::Majority {},
        },
        allow_revoting: false,
        total_power: Uint128::new(1),
        msgs: vec![],
        status: Status::Open,
        veto: None,
        votes: Votes::zero(),
        individual_votes: Votes::zero(),
        delegation_module: None,
    };

    assert_eq!(created.proposal, expected);
    assert_eq!(created.id, 1u64);

    // Check that the deposit info for this proposal looks right.
    let (_, pre_propose) = query_deposit_config_and_pre_propose_module(&app, &proposal_module);
    let deposit_response = query_pre_proposal_single_deposit_info(&app, &pre_propose, id);

    assert_eq!(deposit_response.proposer, Addr::unchecked(CREATOR_ADDR));
    assert_eq!(deposit_response.deposit_info, None,);
}

#[test]
fn test_simple_proposal_auto_vote_yes() {
    let mut app = App::default();
    let instantiate = get_default_non_token_dao_proposal_module_instantiate(&mut app);
    let core_addr = instantiate_with_cw4_groups_governance(&mut app, instantiate, None);
    let proposal_module = query_single_proposal_module(&app, &core_addr);
    let id = make_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        vec![],
        Some(SingleChoiceAutoVote {
            vote: Vote::Yes,
            rationale: Some("rationale".to_string()),
        }),
    );

    let created = query_proposal(&app, &proposal_module, id);
    let current_block = app.block_info();

    // These values just come from the default instantiate message
    // values.
    let expected = SingleChoiceProposal {
        title: "title".to_string(),
        description: "description".to_string(),
        proposer: Addr::unchecked(CREATOR_ADDR),
        start_height: current_block.height,
        expiration: Duration::Time(604800).after(&current_block),
        min_voting_period: None,
        threshold: Threshold::ThresholdQuorum {
            threshold: PercentageThreshold::Percent(Decimal::percent(15)),
            quorum: PercentageThreshold::Majority {},
        },
        allow_revoting: false,
        total_power: Uint128::new(1),
        msgs: vec![],
        status: Status::Passed,
        veto: None,
        votes: Votes {
            yes: Uint128::new(1),
            no: Uint128::zero(),
            abstain: Uint128::zero(),
        },
        individual_votes: Votes {
            yes: Uint128::new(1),
            no: Uint128::zero(),
            abstain: Uint128::zero(),
        },
        delegation_module: None,
    };

    assert_eq!(created.proposal, expected);
    assert_eq!(created.id, 1u64);
}

#[test]
fn test_simple_proposal_auto_vote_no() {
    let mut app = App::default();
    let instantiate = get_default_non_token_dao_proposal_module_instantiate(&mut app);
    let core_addr = instantiate_with_cw4_groups_governance(&mut app, instantiate, None);
    let proposal_module = query_single_proposal_module(&app, &core_addr);
    let id = make_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        vec![],
        Some(SingleChoiceAutoVote {
            vote: Vote::No,
            rationale: Some("rationale".to_string()),
        }),
    );

    let created = query_proposal(&app, &proposal_module, id);
    let current_block = app.block_info();

    // These values just come from the default instantiate message
    // values.
    let expected = SingleChoiceProposal {
        title: "title".to_string(),
        description: "description".to_string(),
        proposer: Addr::unchecked(CREATOR_ADDR),
        start_height: current_block.height,
        expiration: Duration::Time(604800).after(&current_block),
        min_voting_period: None,
        threshold: Threshold::ThresholdQuorum {
            threshold: PercentageThreshold::Percent(Decimal::percent(15)),
            quorum: PercentageThreshold::Majority {},
        },
        allow_revoting: false,
        total_power: Uint128::new(1),
        msgs: vec![],
        status: Status::Rejected,
        veto: None,
        votes: Votes {
            yes: Uint128::zero(),
            no: Uint128::new(1),
            abstain: Uint128::zero(),
        },
        individual_votes: Votes {
            yes: Uint128::zero(),
            no: Uint128::new(1),
            abstain: Uint128::zero(),
        },
        delegation_module: None,
    };

    assert_eq!(created.proposal, expected);
    assert_eq!(created.id, 1u64);
}

#[test]
fn test_propose_supports_stargate_messages() {
    // If we can make a proposal with a stargate message, we support
    // stargate messages in proposals.
    setup_test(vec![CosmosMsg::Stargate {
        type_url: "foo_type".to_string(),
        value: Binary::default(),
    }]);
}

/// Test that the deposit token is properly set to the voting module
/// token during instantiation.
#[test]
fn test_voting_module_token_instantiate() {
    let CommonTest {
        app,
        core_addr: _,
        proposal_module,
        gov_token,
        proposal_id,
    } = setup_test(vec![]);

    let (_, pre_propose) = query_deposit_config_and_pre_propose_module(&app, &proposal_module);
    let deposit_response = query_pre_proposal_single_deposit_info(&app, &pre_propose, proposal_id);

    let deposit_token = if let Some(CheckedDepositInfo {
        denom: CheckedDenom::Cw20(addr),
        ..
    }) = deposit_response.deposit_info
    {
        addr
    } else {
        panic!("voting module should have governance token")
    };
    assert_eq!(deposit_token, gov_token)
}

#[test]
#[should_panic(
    expected = "Error parsing into type dao_voting_cw4::msg::QueryMsg: unknown variant `token_contract`"
)]
fn test_deposit_token_voting_module_token_fails_if_no_voting_module_token() {
    let mut app = App::default();
    let instantiate = get_default_token_dao_proposal_module_instantiate(&mut app);
    instantiate_with_cw4_groups_governance(&mut app, instantiate, None);
}

#[test]
fn test_instantiate_with_non_voting_module_cw20_deposit() {
    let mut app = App::default();
    let alt_cw20 = instantiate_cw20_base_default(&mut app);

    let mut instantiate = get_default_non_token_dao_proposal_module_instantiate(&mut app);
    // hehehehehehehehe
    instantiate.pre_propose_info = get_pre_propose_info(
        &mut app,
        Some(UncheckedDepositInfo {
            denom: dao_voting::deposit::DepositToken::Token {
                denom: cw_denom::UncheckedDenom::Cw20(alt_cw20.to_string()),
            },
            amount: Uint128::new(10_000_000),
            refund_policy: dao_voting::deposit::DepositRefundPolicy::OnlyPassed,
        }),
        false,
    );

    let core_addr = instantiate_with_cw4_groups_governance(&mut app, instantiate, None);
    let proposal_module = query_single_proposal_module(&app, &core_addr);
    let proposal_id = make_proposal(&mut app, &proposal_module, CREATOR_ADDR, vec![], None);

    let created = query_proposal(&app, &proposal_module, proposal_id);
    let current_block = app.block_info();

    // These values just come from the default instantiate message
    // values.
    let expected = SingleChoiceProposal {
        title: "title".to_string(),
        description: "description".to_string(),
        proposer: Addr::unchecked(CREATOR_ADDR),
        start_height: current_block.height,
        expiration: Duration::Time(604800).after(&current_block),
        min_voting_period: None,
        threshold: Threshold::ThresholdQuorum {
            threshold: PercentageThreshold::Percent(Decimal::percent(15)),
            quorum: PercentageThreshold::Majority {},
        },
        allow_revoting: false,
        total_power: Uint128::new(1),
        msgs: vec![],
        status: Status::Open,
        votes: Votes::zero(),
        individual_votes: Votes::zero(),
        veto: None,
        delegation_module: None,
    };

    assert_eq!(created.proposal, expected);
    assert_eq!(created.id, 1u64);

    // Check that the deposit info for this proposal looks right.
    let (_, pre_propose) = query_deposit_config_and_pre_propose_module(&app, &proposal_module);
    let deposit_response = query_pre_proposal_single_deposit_info(&app, &pre_propose, proposal_id);

    assert_eq!(deposit_response.proposer, Addr::unchecked(CREATOR_ADDR));
    assert_eq!(
        deposit_response.deposit_info,
        Some(CheckedDepositInfo {
            denom: cw_denom::CheckedDenom::Cw20(alt_cw20),
            amount: Uint128::new(10_000_000),
            refund_policy: dao_voting::deposit::DepositRefundPolicy::OnlyPassed
        })
    );
}

#[test]
fn test_proposal_message_execution() {
    let mut app = App::default();
    let mut instantiate = get_default_token_dao_proposal_module_instantiate(&mut app);
    instantiate.close_proposal_on_execution_failure = false;
    let core_addr = instantiate_with_staked_balances_governance(&mut app, instantiate, None);
    let proposal_module = query_single_proposal_module(&app, &core_addr);
    let gov_token = query_dao_token(&app, &core_addr);

    mint_cw20s(&mut app, &gov_token, &core_addr, CREATOR_ADDR, 10_000_000);
    let proposal_id = make_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        vec![
            WasmMsg::Execute {
                contract_addr: gov_token.to_string(),
                msg: to_json_binary(&cw20::Cw20ExecuteMsg::Mint {
                    recipient: CREATOR_ADDR.to_string(),
                    amount: Uint128::new(10_000_000),
                })
                .unwrap(),
                funds: vec![],
            }
            .into(),
            BankMsg::Send {
                to_address: CREATOR_ADDR.to_string(),
                amount: coins(10, "ujuno"),
            }
            .into(),
        ],
        None,
    );
    let cw20_balance = query_balance_cw20(&app, &gov_token, CREATOR_ADDR);
    let native_balance = query_balance_native(&app, CREATOR_ADDR, "ujuno");
    assert_eq!(cw20_balance, Uint128::zero());
    assert_eq!(native_balance, Uint128::zero());

    vote_on_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        proposal_id,
        Vote::Yes,
    );
    let proposal = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal.proposal.status, Status::Passed);

    // Can't use library function because we expect this to fail due
    // to insufficent balance in the bank module.
    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        proposal_module.clone(),
        &ExecuteMsg::Execute { proposal_id },
        &[],
    )
    .unwrap_err();
    let proposal = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal.proposal.status, Status::Passed);

    mint_natives(&mut app, core_addr.as_str(), coins(10, "ujuno"));
    execute_proposal(&mut app, &proposal_module, CREATOR_ADDR, proposal_id);
    let proposal = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal.proposal.status, Status::Executed);

    let cw20_balance = query_balance_cw20(&app, &gov_token, CREATOR_ADDR);
    let native_balance = query_balance_native(&app, CREATOR_ADDR, "ujuno");
    assert_eq!(cw20_balance, Uint128::new(20_000_000));
    assert_eq!(native_balance, Uint128::new(10));

    // Sneak in a check here that proposals can't be executed more
    // than once in the on close on execute config suituation.
    let err = execute_proposal_should_fail(&mut app, &proposal_module, CREATOR_ADDR, proposal_id);
    assert!(matches!(err, ContractError::NotPassed {}))
}

#[test]
fn test_proposal_message_timelock_execution() -> anyhow::Result<()> {
    let mut app = App::default();
    let mut instantiate = get_default_token_dao_proposal_module_instantiate(&mut app);
    let veto_config = VetoConfig {
        timelock_duration: Duration::Time(100),
        vetoer: "oversight".to_string(),
        early_execute: false,
        veto_before_passed: false,
    };
    instantiate.close_proposal_on_execution_failure = false;
    instantiate.veto = Some(veto_config.clone());
    let core_addr = instantiate_with_staked_balances_governance(
        &mut app,
        instantiate,
        Some(vec![
            Cw20Coin {
                address: "oversight".to_string(),
                amount: Uint128::new(15),
            },
            Cw20Coin {
                address: CREATOR_ADDR.to_string(),
                amount: Uint128::new(85),
            },
        ]),
    );
    let proposal_module = query_single_proposal_module(&app, &core_addr);
    let gov_token = query_dao_token(&app, &core_addr);

    mint_cw20s(&mut app, &gov_token, &core_addr, CREATOR_ADDR, 10_000_000);
    let proposal_id = make_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        vec![
            WasmMsg::Execute {
                contract_addr: gov_token.to_string(),
                msg: to_json_binary(&cw20::Cw20ExecuteMsg::Mint {
                    recipient: CREATOR_ADDR.to_string(),
                    amount: Uint128::new(10_000_000),
                })
                .unwrap(),
                funds: vec![],
            }
            .into(),
            BankMsg::Send {
                to_address: CREATOR_ADDR.to_string(),
                amount: coins(10, "ujuno"),
            }
            .into(),
        ],
        None,
    );
    let cw20_balance = query_balance_cw20(&app, &gov_token, CREATOR_ADDR);
    let native_balance = query_balance_native(&app, CREATOR_ADDR, "ujuno");
    assert_eq!(cw20_balance, Uint128::zero());
    assert_eq!(native_balance, Uint128::zero());

    vote_on_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        proposal_id,
        Vote::Yes,
    );
    let proposal = query_proposal(&app, &proposal_module, proposal_id);

    // Proposal is timelocked to the moment of prop expiring + timelock delay
    assert_eq!(
        proposal.proposal.status,
        Status::VetoTimelock {
            expiration: proposal
                .proposal
                .expiration
                .add(veto_config.timelock_duration)?,
        }
    );

    mint_natives(&mut app, core_addr.as_str(), coins(10, "ujuno"));

    // vetoer can't execute when timelock is active and
    // early execute not enabled.
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked("oversight"),
            proposal_module.clone(),
            &ExecuteMsg::Execute { proposal_id },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::VetoError(VetoError::NoEarlyExecute {}));

    // Proposal cannot be excuted before timelock expires
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            proposal_module.clone(),
            &ExecuteMsg::Execute { proposal_id },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert_eq!(err, ContractError::VetoError(VetoError::Timelocked {}));

    // Time passes
    app.update_block(|block| {
        block.time = block.time.plus_seconds(604800 + 200);
    });

    // Proposal executes successfully
    execute_proposal(&mut app, &proposal_module, CREATOR_ADDR, proposal_id);
    let proposal = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal.proposal.status, Status::Executed);

    Ok(())
}

// only the authorized vetoer can veto an open proposal
#[test]
fn test_open_proposal_veto_unauthorized() {
    let mut app = App::default();
    let mut instantiate = get_default_token_dao_proposal_module_instantiate(&mut app);
    instantiate.close_proposal_on_execution_failure = false;
    let veto_config = VetoConfig {
        timelock_duration: Duration::Time(100),
        vetoer: "oversight".to_string(),
        early_execute: false,
        veto_before_passed: true,
    };
    instantiate.veto = Some(veto_config.clone());
    let core_addr = instantiate_with_staked_balances_governance(
        &mut app,
        instantiate,
        Some(vec![Cw20Coin {
            address: CREATOR_ADDR.to_string(),
            amount: Uint128::new(85),
        }]),
    );
    let proposal_module = query_single_proposal_module(&app, &core_addr);
    let gov_token = query_dao_token(&app, &core_addr);

    mint_cw20s(&mut app, &gov_token, &core_addr, CREATOR_ADDR, 10_000_000);
    let proposal_id = make_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        vec![
            WasmMsg::Execute {
                contract_addr: gov_token.to_string(),
                msg: to_json_binary(&cw20::Cw20ExecuteMsg::Mint {
                    recipient: CREATOR_ADDR.to_string(),
                    amount: Uint128::new(10_000_000),
                })
                .unwrap(),
                funds: vec![],
            }
            .into(),
            BankMsg::Send {
                to_address: CREATOR_ADDR.to_string(),
                amount: coins(10, "ujuno"),
            }
            .into(),
        ],
        None,
    );

    // only the vetoer can veto
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked("not-oversight"),
            proposal_module.clone(),
            &ExecuteMsg::Veto { proposal_id },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::VetoError(VetoError::Unauthorized {}));
}

// open proposal can only be vetoed if `veto_before_passed` flag is enabled
#[test]
fn test_open_proposal_veto_with_early_veto_flag_disabled() {
    let mut app = App::default();
    let mut instantiate = get_default_token_dao_proposal_module_instantiate(&mut app);
    instantiate.close_proposal_on_execution_failure = false;
    let veto_config = VetoConfig {
        timelock_duration: Duration::Time(100),
        vetoer: "oversight".to_string(),
        early_execute: false,
        veto_before_passed: false,
    };
    instantiate.veto = Some(veto_config.clone());
    let core_addr = instantiate_with_staked_balances_governance(
        &mut app,
        instantiate,
        Some(vec![Cw20Coin {
            address: CREATOR_ADDR.to_string(),
            amount: Uint128::new(85),
        }]),
    );
    let proposal_module = query_single_proposal_module(&app, &core_addr);
    let gov_token = query_dao_token(&app, &core_addr);

    mint_cw20s(&mut app, &gov_token, &core_addr, CREATOR_ADDR, 10_000_000);
    let proposal_id = make_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        vec![
            WasmMsg::Execute {
                contract_addr: gov_token.to_string(),
                msg: to_json_binary(&cw20::Cw20ExecuteMsg::Mint {
                    recipient: CREATOR_ADDR.to_string(),
                    amount: Uint128::new(10_000_000),
                })
                .unwrap(),
                funds: vec![],
            }
            .into(),
            BankMsg::Send {
                to_address: CREATOR_ADDR.to_string(),
                amount: coins(10, "ujuno"),
            }
            .into(),
        ],
        None,
    );

    let err: ContractError = app
        .execute_contract(
            Addr::unchecked("oversight"),
            proposal_module.clone(),
            &ExecuteMsg::Veto { proposal_id },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(
        err,
        ContractError::VetoError(VetoError::NoVetoBeforePassed {})
    );
}

#[test]
fn test_open_proposal_veto_with_no_timelock() {
    let mut app = App::default();
    let mut instantiate = get_default_token_dao_proposal_module_instantiate(&mut app);
    instantiate.close_proposal_on_execution_failure = false;
    instantiate.veto = None;
    let core_addr = instantiate_with_staked_balances_governance(
        &mut app,
        instantiate,
        Some(vec![Cw20Coin {
            address: CREATOR_ADDR.to_string(),
            amount: Uint128::new(85),
        }]),
    );
    let proposal_module = query_single_proposal_module(&app, &core_addr);
    let gov_token = query_dao_token(&app, &core_addr);

    mint_cw20s(&mut app, &gov_token, &core_addr, CREATOR_ADDR, 10_000_000);
    let proposal_id = make_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        vec![
            WasmMsg::Execute {
                contract_addr: gov_token.to_string(),
                msg: to_json_binary(&cw20::Cw20ExecuteMsg::Mint {
                    recipient: CREATOR_ADDR.to_string(),
                    amount: Uint128::new(10_000_000),
                })
                .unwrap(),
                funds: vec![],
            }
            .into(),
            BankMsg::Send {
                to_address: CREATOR_ADDR.to_string(),
                amount: coins(10, "ujuno"),
            }
            .into(),
        ],
        None,
    );

    let err: ContractError = app
        .execute_contract(
            Addr::unchecked("oversight"),
            proposal_module.clone(),
            &ExecuteMsg::Veto { proposal_id },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(
        err,
        ContractError::VetoError(VetoError::NoVetoConfiguration {})
    );
}

// if proposal is not open or timelocked, attempts to veto should
// throw an error
#[test]
fn test_vetoed_proposal_veto() {
    let mut app = App::default();
    let mut instantiate = get_default_token_dao_proposal_module_instantiate(&mut app);
    instantiate.close_proposal_on_execution_failure = false;
    let veto_config = VetoConfig {
        timelock_duration: Duration::Time(100),
        vetoer: "oversight".to_string(),
        early_execute: false,
        veto_before_passed: true,
    };
    instantiate.veto = Some(veto_config.clone());
    let core_addr = instantiate_with_staked_balances_governance(
        &mut app,
        instantiate,
        Some(vec![Cw20Coin {
            address: CREATOR_ADDR.to_string(),
            amount: Uint128::new(85),
        }]),
    );
    let proposal_module = query_single_proposal_module(&app, &core_addr);
    let gov_token = query_dao_token(&app, &core_addr);

    mint_cw20s(&mut app, &gov_token, &core_addr, CREATOR_ADDR, 10_000_000);
    let proposal_id = make_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        vec![
            WasmMsg::Execute {
                contract_addr: gov_token.to_string(),
                msg: to_json_binary(&cw20::Cw20ExecuteMsg::Mint {
                    recipient: CREATOR_ADDR.to_string(),
                    amount: Uint128::new(10_000_000),
                })
                .unwrap(),
                funds: vec![],
            }
            .into(),
            BankMsg::Send {
                to_address: CREATOR_ADDR.to_string(),
                amount: coins(10, "ujuno"),
            }
            .into(),
        ],
        None,
    );

    app.execute_contract(
        Addr::unchecked("oversight"),
        proposal_module.clone(),
        &ExecuteMsg::Veto { proposal_id },
        &[],
    )
    .unwrap();

    let proposal = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal.proposal.status, Status::Vetoed {});

    let err: ContractError = app
        .execute_contract(
            Addr::unchecked("oversight"),
            proposal_module.clone(),
            &ExecuteMsg::Veto { proposal_id },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert_eq!(
        ContractError::VetoError(VetoError::InvalidProposalStatus {
            status: "vetoed".to_string()
        }),
        err,
    );
}

#[test]
fn test_open_proposal_veto_early() {
    let mut app = App::default();
    let mut instantiate = get_default_token_dao_proposal_module_instantiate(&mut app);
    instantiate.close_proposal_on_execution_failure = false;
    let veto_config = VetoConfig {
        timelock_duration: Duration::Time(100),
        vetoer: "oversight".to_string(),
        early_execute: false,
        veto_before_passed: true,
    };
    instantiate.veto = Some(veto_config.clone());
    let core_addr = instantiate_with_staked_balances_governance(
        &mut app,
        instantiate,
        Some(vec![Cw20Coin {
            address: CREATOR_ADDR.to_string(),
            amount: Uint128::new(85),
        }]),
    );
    let proposal_module = query_single_proposal_module(&app, &core_addr);
    let gov_token = query_dao_token(&app, &core_addr);

    mint_cw20s(&mut app, &gov_token, &core_addr, CREATOR_ADDR, 10_000_000);
    let proposal_id = make_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        vec![
            WasmMsg::Execute {
                contract_addr: gov_token.to_string(),
                msg: to_json_binary(&cw20::Cw20ExecuteMsg::Mint {
                    recipient: CREATOR_ADDR.to_string(),
                    amount: Uint128::new(10_000_000),
                })
                .unwrap(),
                funds: vec![],
            }
            .into(),
            BankMsg::Send {
                to_address: CREATOR_ADDR.to_string(),
                amount: coins(10, "ujuno"),
            }
            .into(),
        ],
        None,
    );

    app.execute_contract(
        Addr::unchecked("oversight"),
        proposal_module.clone(),
        &ExecuteMsg::Veto { proposal_id },
        &[],
    )
    .unwrap();

    let proposal = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal.proposal.status, Status::Vetoed {});
}

// only the vetoer can veto during timelock period
#[test]
fn test_timelocked_proposal_veto_unauthorized() -> anyhow::Result<()> {
    let mut app = App::default();
    let mut instantiate = get_default_token_dao_proposal_module_instantiate(&mut app);
    instantiate.close_proposal_on_execution_failure = false;
    let veto_config = VetoConfig {
        timelock_duration: Duration::Time(100),
        vetoer: "oversight".to_string(),
        early_execute: true,
        veto_before_passed: false,
    };
    instantiate.veto = Some(veto_config.clone());
    let core_addr = instantiate_with_staked_balances_governance(
        &mut app,
        instantiate,
        Some(vec![
            Cw20Coin {
                address: "oversight".to_string(),
                amount: Uint128::new(15),
            },
            Cw20Coin {
                address: CREATOR_ADDR.to_string(),
                amount: Uint128::new(85),
            },
        ]),
    );
    let proposal_module = query_single_proposal_module(&app, &core_addr);
    let gov_token = query_dao_token(&app, &core_addr);

    mint_cw20s(&mut app, &gov_token, &core_addr, CREATOR_ADDR, 10_000_000);
    let proposal_id = make_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        vec![
            WasmMsg::Execute {
                contract_addr: gov_token.to_string(),
                msg: to_json_binary(&cw20::Cw20ExecuteMsg::Mint {
                    recipient: CREATOR_ADDR.to_string(),
                    amount: Uint128::new(10_000_000),
                })
                .unwrap(),
                funds: vec![],
            }
            .into(),
            BankMsg::Send {
                to_address: CREATOR_ADDR.to_string(),
                amount: coins(10, "ujuno"),
            }
            .into(),
        ],
        None,
    );

    vote_on_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        proposal_id,
        Vote::Yes,
    );
    let proposal = query_proposal(&app, &proposal_module, proposal_id);

    // Proposal is timelocked to the moment of prop expiring + timelock delay
    assert_eq!(
        proposal.proposal.status,
        Status::VetoTimelock {
            expiration: proposal
                .proposal
                .expiration
                .add(veto_config.timelock_duration)?,
        }
    );

    let err: ContractError = app
        .execute_contract(
            Addr::unchecked("not-oversight"),
            proposal_module.clone(),
            &ExecuteMsg::Veto { proposal_id },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert_eq!(err, ContractError::VetoError(VetoError::Unauthorized {}),);
    let proposal = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(
        proposal.proposal.status,
        Status::VetoTimelock {
            expiration: proposal
                .proposal
                .expiration
                .add(veto_config.timelock_duration)?,
        }
    );

    Ok(())
}

// vetoer can only veto the proposal before the timelock expires
#[test]
fn test_timelocked_proposal_veto_expired_timelock() -> anyhow::Result<()> {
    let mut app = App::default();
    let mut instantiate = get_default_token_dao_proposal_module_instantiate(&mut app);
    instantiate.close_proposal_on_execution_failure = false;
    let veto_config = VetoConfig {
        timelock_duration: Duration::Time(100),
        vetoer: "oversight".to_string(),
        early_execute: true,
        veto_before_passed: false,
    };
    instantiate.veto = Some(veto_config.clone());
    let core_addr = instantiate_with_staked_balances_governance(
        &mut app,
        instantiate,
        Some(vec![
            Cw20Coin {
                address: "oversight".to_string(),
                amount: Uint128::new(15),
            },
            Cw20Coin {
                address: CREATOR_ADDR.to_string(),
                amount: Uint128::new(85),
            },
        ]),
    );
    let proposal_module = query_single_proposal_module(&app, &core_addr);
    let gov_token = query_dao_token(&app, &core_addr);

    mint_cw20s(&mut app, &gov_token, &core_addr, CREATOR_ADDR, 10_000_000);
    let proposal_id = make_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        vec![
            WasmMsg::Execute {
                contract_addr: gov_token.to_string(),
                msg: to_json_binary(&cw20::Cw20ExecuteMsg::Mint {
                    recipient: CREATOR_ADDR.to_string(),
                    amount: Uint128::new(10_000_000),
                })
                .unwrap(),
                funds: vec![],
            }
            .into(),
            BankMsg::Send {
                to_address: CREATOR_ADDR.to_string(),
                amount: coins(10, "ujuno"),
            }
            .into(),
        ],
        None,
    );

    vote_on_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        proposal_id,
        Vote::Yes,
    );
    let proposal = query_proposal(&app, &proposal_module, proposal_id);

    // Proposal is timelocked to the moment of prop expiring + timelock delay
    assert_eq!(
        proposal.proposal.status,
        Status::VetoTimelock {
            expiration: proposal
                .proposal
                .expiration
                .add(veto_config.timelock_duration)?,
        }
    );
    app.update_block(|b| b.time = b.time.plus_seconds(604800 + 200));

    let err: ContractError = app
        .execute_contract(
            Addr::unchecked("oversight"),
            proposal_module.clone(),
            &ExecuteMsg::Veto { proposal_id },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert_eq!(err, ContractError::VetoError(VetoError::TimelockExpired {}),);

    Ok(())
}

// vetoer can only exec timelocked prop if the early exec flag is enabled
#[test]
fn test_timelocked_proposal_execute_no_early_exec() -> anyhow::Result<()> {
    let mut app = App::default();
    let mut instantiate = get_default_token_dao_proposal_module_instantiate(&mut app);
    instantiate.close_proposal_on_execution_failure = false;
    let veto_config = VetoConfig {
        timelock_duration: Duration::Time(100),
        vetoer: "oversight".to_string(),
        early_execute: false,
        veto_before_passed: false,
    };
    instantiate.veto = Some(veto_config.clone());
    let core_addr = instantiate_with_staked_balances_governance(
        &mut app,
        instantiate,
        Some(vec![Cw20Coin {
            address: CREATOR_ADDR.to_string(),
            amount: Uint128::new(85),
        }]),
    );
    let proposal_module = query_single_proposal_module(&app, &core_addr);
    let gov_token = query_dao_token(&app, &core_addr);

    mint_cw20s(&mut app, &gov_token, &core_addr, CREATOR_ADDR, 10_000_000);
    let proposal_id = make_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        vec![
            WasmMsg::Execute {
                contract_addr: gov_token.to_string(),
                msg: to_json_binary(&cw20::Cw20ExecuteMsg::Mint {
                    recipient: CREATOR_ADDR.to_string(),
                    amount: Uint128::new(10_000_000),
                })
                .unwrap(),
                funds: vec![],
            }
            .into(),
            BankMsg::Send {
                to_address: CREATOR_ADDR.to_string(),
                amount: coins(10, "ujuno"),
            }
            .into(),
        ],
        None,
    );

    vote_on_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        proposal_id,
        Vote::Yes,
    );
    let proposal = query_proposal(&app, &proposal_module, proposal_id);

    // Proposal is timelocked to the moment of prop expiring + timelock delay
    assert_eq!(
        proposal.proposal.status,
        Status::VetoTimelock {
            expiration: proposal
                .proposal
                .expiration
                .add(veto_config.timelock_duration)?,
        }
    );

    let err: ContractError = app
        .execute_contract(
            Addr::unchecked("oversight"),
            proposal_module.clone(),
            &ExecuteMsg::Execute { proposal_id },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert_eq!(err, ContractError::VetoError(VetoError::NoEarlyExecute {}),);

    Ok(())
}

#[test]
fn test_timelocked_proposal_execute_early() -> anyhow::Result<()> {
    let mut app = App::default();
    let mut instantiate = get_default_token_dao_proposal_module_instantiate(&mut app);
    instantiate.close_proposal_on_execution_failure = false;
    let veto_config = VetoConfig {
        timelock_duration: Duration::Time(100),
        vetoer: "oversight".to_string(),
        early_execute: true,
        veto_before_passed: false,
    };
    instantiate.veto = Some(veto_config.clone());
    let core_addr = instantiate_with_staked_balances_governance(
        &mut app,
        instantiate,
        Some(vec![Cw20Coin {
            address: CREATOR_ADDR.to_string(),
            amount: Uint128::new(85),
        }]),
    );
    let proposal_module = query_single_proposal_module(&app, &core_addr);
    let gov_token = query_dao_token(&app, &core_addr);

    mint_cw20s(&mut app, &gov_token, &core_addr, CREATOR_ADDR, 10_000_000);
    let proposal_id = make_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        vec![
            WasmMsg::Execute {
                contract_addr: gov_token.to_string(),
                msg: to_json_binary(&cw20::Cw20ExecuteMsg::Mint {
                    recipient: CREATOR_ADDR.to_string(),
                    amount: Uint128::new(10_000_000),
                })
                .unwrap(),
                funds: vec![],
            }
            .into(),
            BankMsg::Send {
                to_address: CREATOR_ADDR.to_string(),
                amount: coins(10, "ujuno"),
            }
            .into(),
        ],
        None,
    );

    vote_on_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        proposal_id,
        Vote::Yes,
    );
    let proposal = query_proposal(&app, &proposal_module, proposal_id);

    // Proposal is timelocked to the moment of prop expiring + timelock delay
    assert_eq!(
        proposal.proposal.status,
        Status::VetoTimelock {
            expiration: proposal
                .proposal
                .expiration
                .add(veto_config.timelock_duration)?,
        }
    );

    // assert timelock is active
    assert!(!veto_config
        .timelock_duration
        .after(&app.block_info())
        .is_expired(&app.block_info()));
    mint_natives(&mut app, core_addr.as_str(), coins(10, "ujuno"));

    app.execute_contract(
        Addr::unchecked("oversight"),
        proposal_module.clone(),
        &ExecuteMsg::Execute { proposal_id },
        &[],
    )
    .unwrap();

    let proposal = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal.proposal.status, Status::Executed {});

    Ok(())
}

// only vetoer can exec timelocked prop early
#[test]
fn test_timelocked_proposal_execute_active_timelock_unauthorized() -> anyhow::Result<()> {
    let mut app = App::default();
    let mut instantiate = get_default_token_dao_proposal_module_instantiate(&mut app);
    instantiate.close_proposal_on_execution_failure = false;
    let veto_config = VetoConfig {
        timelock_duration: Duration::Time(100),
        vetoer: "oversight".to_string(),
        early_execute: true,
        veto_before_passed: false,
    };
    instantiate.veto = Some(veto_config.clone());
    let core_addr = instantiate_with_staked_balances_governance(
        &mut app,
        instantiate,
        Some(vec![Cw20Coin {
            address: CREATOR_ADDR.to_string(),
            amount: Uint128::new(85),
        }]),
    );
    let proposal_module = query_single_proposal_module(&app, &core_addr);
    let gov_token = query_dao_token(&app, &core_addr);

    mint_cw20s(&mut app, &gov_token, &core_addr, CREATOR_ADDR, 10_000_000);
    let proposal_id = make_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        vec![
            WasmMsg::Execute {
                contract_addr: gov_token.to_string(),
                msg: to_json_binary(&cw20::Cw20ExecuteMsg::Mint {
                    recipient: CREATOR_ADDR.to_string(),
                    amount: Uint128::new(10_000_000),
                })
                .unwrap(),
                funds: vec![],
            }
            .into(),
            BankMsg::Send {
                to_address: CREATOR_ADDR.to_string(),
                amount: coins(10, "ujuno"),
            }
            .into(),
        ],
        None,
    );

    vote_on_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        proposal_id,
        Vote::Yes,
    );
    let proposal = query_proposal(&app, &proposal_module, proposal_id);

    // Proposal is timelocked to the moment of prop expiring + timelock delay
    assert_eq!(
        proposal.proposal.status,
        Status::VetoTimelock {
            expiration: proposal
                .proposal
                .expiration
                .add(veto_config.timelock_duration)?,
        }
    );

    // assert timelock is active
    assert!(!veto_config
        .timelock_duration
        .after(&app.block_info())
        .is_expired(&app.block_info()));

    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            proposal_module.clone(),
            &ExecuteMsg::Execute { proposal_id },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert_eq!(err, ContractError::VetoError(VetoError::Timelocked {}),);

    Ok(())
}

// anyone can exec the prop after the timelock expires
#[test]
fn test_timelocked_proposal_execute_expired_timelock_not_vetoer() -> anyhow::Result<()> {
    let mut app = App::default();
    let mut instantiate = get_default_token_dao_proposal_module_instantiate(&mut app);
    instantiate.close_proposal_on_execution_failure = false;
    let veto_config = VetoConfig {
        timelock_duration: Duration::Time(100),
        vetoer: "oversight".to_string(),
        early_execute: true,
        veto_before_passed: false,
    };
    instantiate.veto = Some(veto_config.clone());
    let core_addr = instantiate_with_staked_balances_governance(
        &mut app,
        instantiate,
        Some(vec![Cw20Coin {
            address: CREATOR_ADDR.to_string(),
            amount: Uint128::new(85),
        }]),
    );
    let proposal_module = query_single_proposal_module(&app, &core_addr);
    let gov_token = query_dao_token(&app, &core_addr);

    mint_cw20s(&mut app, &gov_token, &core_addr, CREATOR_ADDR, 10_000_000);
    let proposal_id = make_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        vec![
            WasmMsg::Execute {
                contract_addr: gov_token.to_string(),
                msg: to_json_binary(&cw20::Cw20ExecuteMsg::Mint {
                    recipient: CREATOR_ADDR.to_string(),
                    amount: Uint128::new(10_000_000),
                })
                .unwrap(),
                funds: vec![],
            }
            .into(),
            BankMsg::Send {
                to_address: CREATOR_ADDR.to_string(),
                amount: coins(10, "ujuno"),
            }
            .into(),
        ],
        None,
    );

    vote_on_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        proposal_id,
        Vote::Yes,
    );
    let proposal = query_proposal(&app, &proposal_module, proposal_id);

    // Proposal is timelocked to the moment of prop expiring + timelock delay
    let expiration = proposal
        .proposal
        .expiration
        .add(veto_config.timelock_duration)?;
    assert_eq!(
        proposal.proposal.status,
        Status::VetoTimelock { expiration }
    );

    app.update_block(|b| b.time = b.time.plus_seconds(604800 + 201));
    // assert timelock is expired
    assert!(expiration.is_expired(&app.block_info()));
    mint_natives(&mut app, core_addr.as_str(), coins(10, "ujuno"));

    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        proposal_module.clone(),
        &ExecuteMsg::Execute { proposal_id },
        &[],
    )
    .unwrap();

    let proposal = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal.proposal.status, Status::Executed {},);

    Ok(())
}

#[test]
fn test_proposal_message_timelock_veto() -> anyhow::Result<()> {
    let mut app = App::default();
    let mut instantiate = get_default_token_dao_proposal_module_instantiate(&mut app);
    instantiate.close_proposal_on_execution_failure = false;
    let veto_config = VetoConfig {
        timelock_duration: Duration::Time(100),
        vetoer: "oversight".to_string(),
        early_execute: false,
        veto_before_passed: false,
    };
    instantiate.veto = Some(veto_config.clone());
    let core_addr = instantiate_with_staked_balances_governance(
        &mut app,
        instantiate,
        Some(vec![Cw20Coin {
            address: CREATOR_ADDR.to_string(),
            amount: Uint128::new(85),
        }]),
    );
    let proposal_module = query_single_proposal_module(&app, &core_addr);
    let gov_token = query_dao_token(&app, &core_addr);

    mint_cw20s(&mut app, &gov_token, &core_addr, CREATOR_ADDR, 10_000_000);
    let proposal_id = make_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        vec![
            WasmMsg::Execute {
                contract_addr: gov_token.to_string(),
                msg: to_json_binary(&cw20::Cw20ExecuteMsg::Mint {
                    recipient: CREATOR_ADDR.to_string(),
                    amount: Uint128::new(10_000_000),
                })
                .unwrap(),
                funds: vec![],
            }
            .into(),
            BankMsg::Send {
                to_address: CREATOR_ADDR.to_string(),
                amount: coins(10, "ujuno"),
            }
            .into(),
        ],
        None,
    );
    let cw20_balance = query_balance_cw20(&app, &gov_token, CREATOR_ADDR);
    let native_balance = query_balance_native(&app, CREATOR_ADDR, "ujuno");
    assert_eq!(cw20_balance, Uint128::zero());
    assert_eq!(native_balance, Uint128::zero());

    // Vetoer can't veto early
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked("oversight"),
            proposal_module.clone(),
            &ExecuteMsg::Veto { proposal_id },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(
        err,
        ContractError::VetoError(VetoError::NoVetoBeforePassed {})
    );

    // Vote on proposal to pass it
    vote_on_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        proposal_id,
        Vote::Yes,
    );
    let proposal = query_proposal(&app, &proposal_module, proposal_id);

    // Proposal is timelocked to the moment of prop expiring + timelock delay
    assert_eq!(
        proposal.proposal.status,
        Status::VetoTimelock {
            expiration: proposal
                .proposal
                .expiration
                .add(veto_config.timelock_duration)?,
        }
    );

    mint_natives(&mut app, core_addr.as_str(), coins(10, "ujuno"));

    // Non-vetoer cannot veto
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            proposal_module.clone(),
            &ExecuteMsg::Veto { proposal_id },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::VetoError(VetoError::Unauthorized {}));

    // Oversite vetos prop
    app.execute_contract(
        Addr::unchecked("oversight"),
        proposal_module.clone(),
        &ExecuteMsg::Veto { proposal_id },
        &[],
    )
    .unwrap();

    let proposal = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal.proposal.status, Status::Vetoed);

    Ok(())
}

#[test]
fn test_proposal_message_timelock_early_execution() -> anyhow::Result<()> {
    let mut app = App::default();
    let mut instantiate = get_default_token_dao_proposal_module_instantiate(&mut app);
    instantiate.close_proposal_on_execution_failure = false;
    let veto_config = VetoConfig {
        timelock_duration: Duration::Time(100),
        vetoer: "oversight".to_string(),
        early_execute: true,
        veto_before_passed: false,
    };
    instantiate.veto = Some(veto_config.clone());
    let core_addr = instantiate_with_staked_balances_governance(
        &mut app,
        instantiate,
        Some(vec![
            Cw20Coin {
                address: "oversight".to_string(),
                amount: Uint128::new(15),
            },
            Cw20Coin {
                address: CREATOR_ADDR.to_string(),
                amount: Uint128::new(85),
            },
        ]),
    );
    let proposal_module = query_single_proposal_module(&app, &core_addr);
    let gov_token = query_dao_token(&app, &core_addr);

    mint_cw20s(&mut app, &gov_token, &core_addr, CREATOR_ADDR, 10_000_000);
    let proposal_id = make_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        vec![
            WasmMsg::Execute {
                contract_addr: gov_token.to_string(),
                msg: to_json_binary(&cw20::Cw20ExecuteMsg::Mint {
                    recipient: CREATOR_ADDR.to_string(),
                    amount: Uint128::new(10_000_000),
                })
                .unwrap(),
                funds: vec![],
            }
            .into(),
            BankMsg::Send {
                to_address: CREATOR_ADDR.to_string(),
                amount: coins(10, "ujuno"),
            }
            .into(),
        ],
        None,
    );
    let cw20_balance = query_balance_cw20(&app, &gov_token, CREATOR_ADDR);
    let native_balance = query_balance_native(&app, CREATOR_ADDR, "ujuno");
    assert_eq!(cw20_balance, Uint128::zero());
    assert_eq!(native_balance, Uint128::zero());

    vote_on_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        proposal_id,
        Vote::Yes,
    );
    let proposal = query_proposal(&app, &proposal_module, proposal_id);

    // Proposal is timelocked to the moment of prop expiring + timelock delay
    assert_eq!(
        proposal.proposal.status,
        Status::VetoTimelock {
            expiration: proposal
                .proposal
                .expiration
                .add(veto_config.timelock_duration)?,
        }
    );

    mint_natives(&mut app, core_addr.as_str(), coins(10, "ujuno"));

    // Proposal can be executed early by vetoer
    execute_proposal(&mut app, &proposal_module, "oversight", proposal_id);
    let proposal = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal.proposal.status, Status::Executed);

    Ok(())
}

#[test]
fn test_proposal_message_timelock_veto_before_passed() {
    let mut app = App::default();
    let mut instantiate = get_default_token_dao_proposal_module_instantiate(&mut app);
    instantiate.close_proposal_on_execution_failure = false;
    instantiate.veto = Some(VetoConfig {
        timelock_duration: Duration::Time(100),
        vetoer: "oversight".to_string(),
        early_execute: false,
        veto_before_passed: true,
    });
    let core_addr = instantiate_with_staked_balances_governance(
        &mut app,
        instantiate,
        Some(vec![
            Cw20Coin {
                address: "oversight".to_string(),
                amount: Uint128::new(15),
            },
            Cw20Coin {
                address: CREATOR_ADDR.to_string(),
                amount: Uint128::new(85),
            },
        ]),
    );
    let proposal_module = query_single_proposal_module(&app, &core_addr);
    let gov_token = query_dao_token(&app, &core_addr);

    mint_cw20s(&mut app, &gov_token, &core_addr, CREATOR_ADDR, 10_000_000);
    let proposal_id = make_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        vec![
            WasmMsg::Execute {
                contract_addr: gov_token.to_string(),
                msg: to_json_binary(&cw20::Cw20ExecuteMsg::Mint {
                    recipient: CREATOR_ADDR.to_string(),
                    amount: Uint128::new(10_000_000),
                })
                .unwrap(),
                funds: vec![],
            }
            .into(),
            BankMsg::Send {
                to_address: CREATOR_ADDR.to_string(),
                amount: coins(10, "ujuno"),
            }
            .into(),
        ],
        None,
    );

    let proposal = query_proposal(&app, &proposal_module, proposal_id);

    // Proposal is open for voting
    assert_eq!(proposal.proposal.status, Status::Open);

    // Oversite vetos prop
    app.execute_contract(
        Addr::unchecked("oversight"),
        proposal_module.clone(),
        &ExecuteMsg::Veto { proposal_id },
        &[],
    )
    .unwrap();

    let proposal = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal.proposal.status, Status::Vetoed);

    // mint_natives(&mut app, core_addr.as_str(), coins(10, "ujuno"));

    // // Proposal can be executed early by vetoer
    // execute_proposal(&mut app, &proposal_module, "oversight", proposal_id);
    // let proposal = query_proposal(&app, &proposal_module, proposal_id);
    // assert_eq!(proposal.proposal.status, Status::Executed);
}

#[test]
fn test_veto_only_members_execute_proposal() -> anyhow::Result<()> {
    let mut app = App::default();
    let mut instantiate = get_default_token_dao_proposal_module_instantiate(&mut app);
    instantiate.close_proposal_on_execution_failure = false;
    let veto_config = VetoConfig {
        timelock_duration: Duration::Time(100),
        vetoer: "oversight".to_string(),
        early_execute: true,
        veto_before_passed: false,
    };
    instantiate.veto = Some(veto_config.clone());
    let core_addr = instantiate_with_staked_balances_governance(
        &mut app,
        instantiate,
        Some(vec![Cw20Coin {
            address: CREATOR_ADDR.to_string(),
            amount: Uint128::new(85),
        }]),
    );
    let proposal_module = query_single_proposal_module(&app, &core_addr);
    let gov_token = query_dao_token(&app, &core_addr);

    mint_cw20s(&mut app, &gov_token, &core_addr, CREATOR_ADDR, 10_000_000);
    let proposal_id = make_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        vec![
            WasmMsg::Execute {
                contract_addr: gov_token.to_string(),
                msg: to_json_binary(&cw20::Cw20ExecuteMsg::Mint {
                    recipient: CREATOR_ADDR.to_string(),
                    amount: Uint128::new(10_000_000),
                })
                .unwrap(),
                funds: vec![],
            }
            .into(),
            BankMsg::Send {
                to_address: CREATOR_ADDR.to_string(),
                amount: coins(10, "ujuno"),
            }
            .into(),
        ],
        None,
    );
    let cw20_balance = query_balance_cw20(&app, &gov_token, CREATOR_ADDR);
    let native_balance = query_balance_native(&app, CREATOR_ADDR, "ujuno");
    assert_eq!(cw20_balance, Uint128::zero());
    assert_eq!(native_balance, Uint128::zero());

    vote_on_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        proposal_id,
        Vote::Yes,
    );
    let proposal = query_proposal(&app, &proposal_module, proposal_id);

    // Proposal is timelocked to the moment of prop expiring + timelock delay
    let expiration = proposal
        .proposal
        .expiration
        .add(veto_config.timelock_duration)?;
    assert_eq!(
        proposal.proposal.status,
        Status::VetoTimelock { expiration }
    );

    app.update_block(|b| b.time = b.time.plus_seconds(604800 + 101));
    // assert timelock is expired
    assert!(expiration.is_expired(&app.block_info()));
    mint_natives(&mut app, core_addr.as_str(), coins(10, "ujuno"));

    let proposal = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal.proposal.status, Status::Passed);

    // Proposal cannot be executed by vetoer once timelock expired
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked("oversight"),
            proposal_module.clone(),
            &ExecuteMsg::Execute { proposal_id },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::Unauthorized {});

    // Proposal can be executed by member once timelock expired
    execute_proposal(&mut app, &proposal_module, CREATOR_ADDR, proposal_id);
    let proposal = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal.proposal.status, Status::Executed);

    Ok(())
}

#[test]
fn test_proposal_close_after_expiry() {
    let CommonTest {
        mut app,
        core_addr,
        proposal_module,
        gov_token: _,
        proposal_id,
    } = setup_test(vec![BankMsg::Send {
        to_address: CREATOR_ADDR.to_string(),
        amount: coins(10, "ujuno"),
    }
    .into()]);
    mint_natives(&mut app, core_addr.as_str(), coins(10, "ujuno"));

    // Try and close the proposal. This shoudl fail as the proposal is
    // open.
    let err = close_proposal_should_fail(&mut app, &proposal_module, CREATOR_ADDR, proposal_id);
    assert!(matches!(err, ContractError::WrongCloseStatus {}));

    // Expire the proposal. Now it should be closable.
    app.update_block(|b| b.time = b.time.plus_seconds(604800));
    close_proposal(&mut app, &proposal_module, CREATOR_ADDR, proposal_id);
    let proposal = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal.proposal.status, Status::Closed);
}

#[test]
fn test_proposal_cant_close_after_expiry_is_passed() {
    let mut app = App::default();
    let instantiate = get_default_token_dao_proposal_module_instantiate(&mut app);
    let core_addr = instantiate_with_staked_balances_governance(
        &mut app,
        instantiate,
        Some(vec![
            Cw20Coin {
                address: "quorum".to_string(),
                amount: Uint128::new(15),
            },
            Cw20Coin {
                address: CREATOR_ADDR.to_string(),
                amount: Uint128::new(85),
            },
        ]),
    );
    let proposal_module = query_single_proposal_module(&app, &core_addr);
    let gov_token = query_dao_token(&app, &core_addr);

    mint_cw20s(&mut app, &gov_token, &core_addr, CREATOR_ADDR, 10_000_000);
    mint_natives(&mut app, core_addr.as_str(), coins(10, "ujuno"));
    let proposal_id = make_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        vec![BankMsg::Send {
            to_address: CREATOR_ADDR.to_string(),
            amount: coins(10, "ujuno"),
        }
        .into()],
        None,
    );
    vote_on_proposal(&mut app, &proposal_module, "quorum", proposal_id, Vote::Yes);
    let proposal = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal.proposal.status, Status::Open);

    // Expire the proposal. This should pass it.
    app.update_block(|b| b.time = b.time.plus_seconds(604800));
    let proposal = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal.proposal.status, Status::Passed,);

    // Make sure it can't be closed.
    let err = close_proposal_should_fail(&mut app, &proposal_module, CREATOR_ADDR, proposal_id);
    assert!(matches!(err, ContractError::WrongCloseStatus {}));

    // Executed proposals may not be closed.
    execute_proposal(&mut app, &proposal_module, CREATOR_ADDR, proposal_id);
    let err = close_proposal_should_fail(&mut app, &proposal_module, CREATOR_ADDR, proposal_id);
    assert!(matches!(err, ContractError::WrongCloseStatus {}));
    let balance = query_balance_native(&app, CREATOR_ADDR, "ujuno");
    assert_eq!(balance, Uint128::new(10));
    let err = close_proposal_should_fail(&mut app, &proposal_module, CREATOR_ADDR, proposal_id);
    assert!(matches!(err, ContractError::WrongCloseStatus {}));
}

#[test]
fn test_execute_no_non_passed_execution() {
    let CommonTest {
        mut app,
        core_addr,
        proposal_module,
        gov_token,
        proposal_id,
    } = setup_test(vec![BankMsg::Send {
        to_address: CREATOR_ADDR.to_string(),
        amount: coins(10, "ujuno"),
    }
    .into()]);
    mint_natives(&mut app, core_addr.as_str(), coins(100, "ujuno"));

    let err = execute_proposal_should_fail(&mut app, &proposal_module, CREATOR_ADDR, proposal_id);
    assert!(matches!(err, ContractError::NotPassed {}));

    // Expire the proposal.
    app.update_block(|b| b.time = b.time.plus_seconds(604800));
    let err = execute_proposal_should_fail(&mut app, &proposal_module, CREATOR_ADDR, proposal_id);
    assert!(matches!(err, ContractError::NotPassed {}));

    mint_cw20s(&mut app, &gov_token, &core_addr, CREATOR_ADDR, 10_000_000);
    let proposal_id = make_proposal(&mut app, &proposal_module, CREATOR_ADDR, vec![], None);
    vote_on_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        proposal_id,
        Vote::Yes,
    );
    execute_proposal(&mut app, &proposal_module, CREATOR_ADDR, proposal_id);
    // Can't execute more than once.
    let err = execute_proposal_should_fail(&mut app, &proposal_module, CREATOR_ADDR, proposal_id);
    assert!(matches!(err, ContractError::NotPassed {}));
}

#[test]
fn test_cant_execute_not_member_when_proposal_created() {
    let CommonTest {
        mut app,
        core_addr,
        proposal_module,
        gov_token,
        proposal_id,
    } = setup_test(vec![BankMsg::Send {
        to_address: CREATOR_ADDR.to_string(),
        amount: coins(10, "ujuno"),
    }
    .into()]);
    mint_natives(&mut app, core_addr.as_str(), coins(100, "ujuno"));

    vote_on_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        proposal_id,
        Vote::Yes,
    );

    // Give noah some tokens.
    mint_cw20s(&mut app, &gov_token, &core_addr, "noah", 20_000_000);
    // Have noah stake some.
    let voting_module = query_voting_module(&app, &core_addr);
    let staking_contract: Addr = app
        .wrap()
        .query_wasm_smart(
            voting_module,
            &dao_voting_cw20_staked::msg::QueryMsg::StakingContract {},
        )
        .unwrap();
    app.execute_contract(
        Addr::unchecked("noah"),
        gov_token,
        &cw20::Cw20ExecuteMsg::Send {
            contract: staking_contract.to_string(),
            amount: Uint128::new(10_000_000),
            msg: to_json_binary(&cw20_stake::msg::ReceiveMsg::Stake {}).unwrap(),
        },
        &[],
    )
    .unwrap();
    // Update the block so that the staked balance appears.
    app.update_block(|block| block.height += 1);

    // Can't execute from member who wasn't a member when the proposal was
    // created.
    let err = execute_proposal_should_fail(&mut app, &proposal_module, "noah", proposal_id);
    assert!(matches!(err, ContractError::Unauthorized {}));
}

#[test]
fn test_update_config() {
    let CommonTest {
        mut app,
        core_addr,
        proposal_module,
        gov_token: _,
        proposal_id,
    } = setup_test(vec![]);
    vote_on_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        proposal_id,
        Vote::Yes,
    );
    execute_proposal(&mut app, &proposal_module, CREATOR_ADDR, proposal_id);
    // Make a proposal to update the config.
    let proposal_id = make_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        vec![WasmMsg::Execute {
            contract_addr: proposal_module.to_string(),
            msg: to_json_binary(&ExecuteMsg::UpdateConfig {
                veto: Some(VetoConfig {
                    timelock_duration: Duration::Height(2),
                    vetoer: CREATOR_ADDR.to_string(),
                    early_execute: false,
                    veto_before_passed: false,
                }),
                threshold: Threshold::AbsoluteCount {
                    threshold: Uint128::new(10_000),
                },
                max_voting_period: Duration::Height(6),
                min_voting_period: None,
                only_members_execute: true,
                allow_revoting: false,
                dao: core_addr.to_string(),
                close_proposal_on_execution_failure: false,
            })
            .unwrap(),
            funds: vec![],
        }
        .into()],
        None,
    );
    vote_on_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        proposal_id,
        Vote::Yes,
    );
    execute_proposal(&mut app, &proposal_module, CREATOR_ADDR, proposal_id);

    let config = query_proposal_config(&app, &proposal_module);
    assert_eq!(
        config,
        Config {
            veto: Some(VetoConfig {
                timelock_duration: Duration::Height(2),
                vetoer: CREATOR_ADDR.to_string(),
                early_execute: false,
                veto_before_passed: false,
            }),
            threshold: Threshold::AbsoluteCount {
                threshold: Uint128::new(10_000)
            },
            max_voting_period: Duration::Height(6),
            min_voting_period: None,
            only_members_execute: true,
            allow_revoting: false,
            dao: core_addr.clone(),
            close_proposal_on_execution_failure: false,
        }
    );

    // Check that non-dao address may not update config.
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            proposal_module.clone(),
            &&ExecuteMsg::UpdateConfig {
                veto: None,
                threshold: Threshold::AbsoluteCount {
                    threshold: Uint128::new(10_000),
                },
                max_voting_period: Duration::Height(6),
                min_voting_period: None,
                only_members_execute: true,
                allow_revoting: false,
                dao: core_addr.to_string(),
                close_proposal_on_execution_failure: false,
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert!(matches!(err, ContractError::Unauthorized {}));

    // Check that veto config is validated (mismatching duration units).
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(core_addr.clone()),
            proposal_module,
            &&ExecuteMsg::UpdateConfig {
                veto: Some(VetoConfig {
                    timelock_duration: Duration::Time(100),
                    vetoer: CREATOR_ADDR.to_string(),
                    early_execute: false,
                    veto_before_passed: false,
                }),
                threshold: Threshold::AbsoluteCount {
                    threshold: Uint128::new(10_000),
                },
                max_voting_period: Duration::Height(6),
                min_voting_period: None,
                only_members_execute: true,
                allow_revoting: false,
                dao: core_addr.to_string(),
                close_proposal_on_execution_failure: false,
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert!(matches!(
        err,
        ContractError::VetoError(VetoError::TimelockDurationUnitMismatch {})
    ))
}

#[test]
fn test_anyone_may_propose_and_proposal_listing() {
    let mut app = App::default();
    let mut instantiate = get_default_token_dao_proposal_module_instantiate(&mut app);
    instantiate.pre_propose_info = PreProposeInfo::AnyoneMayPropose {};
    let core_addr = instantiate_with_staked_balances_governance(&mut app, instantiate, None);
    let proposal_module = query_single_proposal_module(&app, &core_addr);

    for addr in 'm'..'z' {
        let addr = addr.to_string().repeat(6);
        let proposal_id = make_proposal(&mut app, &proposal_module, &addr, vec![], None);
        vote_on_proposal(
            &mut app,
            &proposal_module,
            CREATOR_ADDR,
            proposal_id,
            Vote::Yes,
        );
        // Only members can execute still.
        let err = execute_proposal_should_fail(&mut app, &proposal_module, &addr, proposal_id);
        assert!(matches!(err, ContractError::Unauthorized {}));
        execute_proposal(&mut app, &proposal_module, CREATOR_ADDR, proposal_id);
    }

    // Now that we've got all these proposals sitting around, lets
    // test that we can query them.

    let proposals_forward = query_list_proposals(&app, &proposal_module, None, None);
    let mut proposals_reverse = query_list_proposals_reverse(&app, &proposal_module, None, None);
    proposals_reverse.proposals.reverse();
    assert_eq!(proposals_reverse, proposals_forward);

    // Check the proposers and (implicitly) the ordering.
    for (index, addr) in ('m'..'z').enumerate() {
        let addr = addr.to_string().repeat(6);
        assert_eq!(
            proposals_forward.proposals[index].proposal.proposer,
            Addr::unchecked(addr)
        )
    }

    let four_and_five = query_list_proposals(&app, &proposal_module, Some(3), Some(2));
    let mut five_and_four = query_list_proposals_reverse(&app, &proposal_module, Some(6), Some(2));
    five_and_four.proposals.reverse();

    assert_eq!(five_and_four, four_and_five);
    assert_eq!(
        four_and_five.proposals[0].proposal.proposer,
        Addr::unchecked("pppppp")
    );

    let current_block = app.block_info();
    assert_eq!(
        four_and_five.proposals[0],
        ProposalResponse {
            id: 4,
            proposal: SingleChoiceProposal {
                title: "title".to_string(),
                description: "description".to_string(),
                proposer: Addr::unchecked("pppppp"),
                start_height: current_block.height,
                min_voting_period: None,
                expiration: Duration::Time(604800).after(&current_block),
                threshold: Threshold::ThresholdQuorum {
                    quorum: PercentageThreshold::Percent(Decimal::percent(15)),
                    threshold: PercentageThreshold::Majority {},
                },
                allow_revoting: false,
                total_power: Uint128::new(100_000_000),
                msgs: vec![],
                status: Status::Executed,
                votes: Votes {
                    yes: Uint128::new(100_000_000),
                    no: Uint128::zero(),
                    abstain: Uint128::zero()
                },
                individual_votes: Votes {
                    yes: Uint128::new(100_000_000),
                    no: Uint128::zero(),
                    abstain: Uint128::zero()
                },
                veto: None,
                delegation_module: None,
            }
        }
    )
}

#[test]
#[should_panic(expected = "not registered to vote (no voting power) at time of proposal creation")]
fn test_propose_non_member_auto_vote_fails() {
    let mut app = App::default();
    let mut instantiate = get_default_token_dao_proposal_module_instantiate(&mut app);
    instantiate.pre_propose_info = PreProposeInfo::AnyoneMayPropose {};
    let core_addr = instantiate_with_staked_balances_governance(&mut app, instantiate, None);
    let proposal_module = query_single_proposal_module(&app, &core_addr);

    // Should fail if non-member tries to vote on proposal creation.
    make_proposal(
        &mut app,
        &proposal_module,
        "anyone",
        vec![],
        Some(SingleChoiceAutoVote {
            vote: Vote::Yes,
            rationale: Some("rationale".to_string()),
        }),
    );
}

#[test]
fn test_proposal_hook_registration() {
    let CommonTest {
        mut app,
        core_addr,
        proposal_module,
        gov_token: _,
        proposal_id: _,
    } = setup_test(vec![]);

    let proposal_hooks = query_proposal_hooks(&app, &proposal_module);
    assert_eq!(
        proposal_hooks.hooks.len(),
        0,
        "pre-propose deposit module should not show on this listing"
    );

    // non-dao may not add a hook.
    let err =
        add_proposal_hook_should_fail(&mut app, &proposal_module, CREATOR_ADDR, "proposalhook");
    assert!(matches!(err, ContractError::Unauthorized {}));

    add_proposal_hook(
        &mut app,
        &proposal_module,
        core_addr.as_str(),
        "proposalhook",
    );
    let err = add_proposal_hook_should_fail(
        &mut app,
        &proposal_module,
        core_addr.as_str(),
        "proposalhook",
    );
    assert!(matches!(
        err,
        ContractError::HookError(HookError::HookAlreadyRegistered {})
    ));

    let proposal_hooks = query_proposal_hooks(&app, &proposal_module);
    assert_eq!(proposal_hooks.hooks[0], "proposalhook".to_string());

    // Only DAO can remove proposal hooks.
    let err =
        remove_proposal_hook_should_fail(&mut app, &proposal_module, CREATOR_ADDR, "proposalhook");
    assert!(matches!(err, ContractError::Unauthorized {}));
    remove_proposal_hook(
        &mut app,
        &proposal_module,
        core_addr.as_str(),
        "proposalhook",
    );
    let proposal_hooks = query_proposal_hooks(&app, &proposal_module);
    assert_eq!(proposal_hooks.hooks.len(), 0);

    // Can not remove that which does not exist.
    let err = remove_proposal_hook_should_fail(
        &mut app,
        &proposal_module,
        core_addr.as_str(),
        "proposalhook",
    );
    assert!(matches!(
        err,
        ContractError::HookError(HookError::HookNotRegistered {})
    ));
}

#[test]
fn test_vote_hook_registration() {
    let CommonTest {
        mut app,
        core_addr,
        proposal_module,
        gov_token: _,
        proposal_id: _,
    } = setup_test(vec![]);

    let vote_hooks = query_vote_hooks(&app, &proposal_module);
    assert!(vote_hooks.hooks.is_empty(),);

    // non-dao may not add a hook.
    let err = add_vote_hook_should_fail(&mut app, &proposal_module, CREATOR_ADDR, "votehook");
    assert!(matches!(err, ContractError::Unauthorized {}));

    add_vote_hook(&mut app, &proposal_module, core_addr.as_str(), "votehook");

    let vote_hooks = query_vote_hooks(&app, &proposal_module);
    assert_eq!(
        vote_hooks,
        HooksResponse {
            hooks: vec!["votehook".to_string()]
        }
    );

    let err = add_vote_hook_should_fail(&mut app, &proposal_module, core_addr.as_str(), "votehook");
    assert!(matches!(
        err,
        ContractError::HookError(HookError::HookAlreadyRegistered {})
    ));

    let vote_hooks = query_vote_hooks(&app, &proposal_module);
    assert_eq!(vote_hooks.hooks[0], "votehook".to_string());

    // Only DAO can remove vote hooks.
    let err = remove_vote_hook_should_fail(&mut app, &proposal_module, CREATOR_ADDR, "votehook");
    assert!(matches!(err, ContractError::Unauthorized {}));
    remove_vote_hook(&mut app, &proposal_module, core_addr.as_str(), "votehook");

    let vote_hooks = query_vote_hooks(&app, &proposal_module);
    assert!(vote_hooks.hooks.is_empty(),);

    // Can not remove that which does not exist.
    let err =
        remove_vote_hook_should_fail(&mut app, &proposal_module, core_addr.as_str(), "votehook");
    assert!(matches!(
        err,
        ContractError::HookError(HookError::HookNotRegistered {})
    ));
}

#[test]
fn test_active_threshold_absolute() {
    let mut app = App::default();
    let mut instantiate = get_default_token_dao_proposal_module_instantiate(&mut app);
    instantiate.pre_propose_info = PreProposeInfo::AnyoneMayPropose {};
    let core_addr = instantiate_with_staking_active_threshold(
        &mut app,
        instantiate,
        None,
        Some(ActiveThreshold::AbsoluteCount {
            count: Uint128::new(100),
        }),
    );
    let gov_token = query_dao_token(&app, &core_addr);
    let proposal_module = query_single_proposal_module(&app, &core_addr);
    let voting_module = query_voting_module(&app, &core_addr);

    let staking_contract: Addr = app
        .wrap()
        .query_wasm_smart(
            voting_module,
            &dao_voting_cw20_staked::msg::QueryMsg::StakingContract {},
        )
        .unwrap();

    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            proposal_module.clone(),
            &ExecuteMsg::Propose(ProposeMsg {
                title: "title".to_string(),
                description: "description".to_string(),
                msgs: vec![],
                proposer: None,
                vote: None,
            }),
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert!(matches!(err, ContractError::InactiveDao {}));

    let msg = cw20::Cw20ExecuteMsg::Send {
        contract: staking_contract.to_string(),
        amount: Uint128::new(100),
        msg: to_json_binary(&cw20_stake::msg::ReceiveMsg::Stake {}).unwrap(),
    };
    app.execute_contract(Addr::unchecked(CREATOR_ADDR), gov_token, &msg, &[])
        .unwrap();
    app.update_block(next_block);

    // Proposal creation now works as tokens have been staked to reach
    // active threshold.
    make_proposal(&mut app, &proposal_module, CREATOR_ADDR, vec![], None);

    // Unstake some tokens to make it inactive again.
    let msg = cw20_stake::msg::ExecuteMsg::Unstake {
        amount: Uint128::new(50),
    };
    app.execute_contract(Addr::unchecked(CREATOR_ADDR), staking_contract, &msg, &[])
        .unwrap();
    app.update_block(next_block);

    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            proposal_module.clone(),
            &ExecuteMsg::Propose(ProposeMsg {
                title: "title".to_string(),
                description: "description".to_string(),
                msgs: vec![],
                proposer: None,
                vote: None,
            }),
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert!(matches!(err, ContractError::InactiveDao {}));
}

#[test]
fn test_active_threshold_percent() {
    let mut app = App::default();
    let mut instantiate = get_default_token_dao_proposal_module_instantiate(&mut app);
    instantiate.pre_propose_info = PreProposeInfo::AnyoneMayPropose {};
    let core_addr = instantiate_with_staking_active_threshold(
        &mut app,
        instantiate,
        None,
        Some(ActiveThreshold::Percentage {
            percent: Decimal::percent(20),
        }),
    );
    let gov_token = query_dao_token(&app, &core_addr);
    let proposal_module = query_single_proposal_module(&app, &core_addr);
    let voting_module = query_voting_module(&app, &core_addr);

    let staking_contract: Addr = app
        .wrap()
        .query_wasm_smart(
            voting_module,
            &dao_voting_cw20_staked::msg::QueryMsg::StakingContract {},
        )
        .unwrap();

    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            proposal_module.clone(),
            &ExecuteMsg::Propose(ProposeMsg {
                title: "title".to_string(),
                description: "description".to_string(),
                msgs: vec![],
                proposer: None,
                vote: None,
            }),
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert!(matches!(err, ContractError::InactiveDao {}));

    let msg = cw20::Cw20ExecuteMsg::Send {
        contract: staking_contract.to_string(),
        amount: Uint128::new(20_000_000),
        msg: to_json_binary(&cw20_stake::msg::ReceiveMsg::Stake {}).unwrap(),
    };
    app.execute_contract(Addr::unchecked(CREATOR_ADDR), gov_token, &msg, &[])
        .unwrap();
    app.update_block(next_block);

    // Proposal creation now works as tokens have been staked to reach
    // active threshold.
    make_proposal(&mut app, &proposal_module, CREATOR_ADDR, vec![], None);

    // Unstake some tokens to make it inactive again.
    let msg = cw20_stake::msg::ExecuteMsg::Unstake {
        amount: Uint128::new(1), // Only one is needed as we're right
                                 // on the edge. :)
    };
    app.execute_contract(Addr::unchecked(CREATOR_ADDR), staking_contract, &msg, &[])
        .unwrap();
    app.update_block(next_block);

    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            proposal_module.clone(),
            &ExecuteMsg::Propose(ProposeMsg {
                title: "title".to_string(),
                description: "description".to_string(),
                msgs: vec![],
                proposer: None,
                vote: None,
            }),
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert!(matches!(err, ContractError::InactiveDao {}));
}

#[test]
#[should_panic(
    expected = "min_voting_period and max_voting_period must have the same units (height or time)"
)]
fn test_min_duration_unit_missmatch() {
    let mut app = App::default();
    let mut instantiate = get_default_token_dao_proposal_module_instantiate(&mut app);
    instantiate.min_voting_period = Some(Duration::Height(10));
    instantiate_with_staked_balances_governance(&mut app, instantiate, None);
}

#[test]
#[should_panic(expected = "Min voting period must be less than or equal to max voting period")]
fn test_min_duration_larger_than_proposal_duration() {
    let mut app = App::default();
    let mut instantiate = get_default_token_dao_proposal_module_instantiate(&mut app);
    instantiate.min_voting_period = Some(Duration::Time(604801));
    instantiate_with_staked_balances_governance(&mut app, instantiate, None);
}

#[test]
fn test_min_voting_period_no_early_pass() {
    let mut app = App::default();
    let mut instantiate = get_default_token_dao_proposal_module_instantiate(&mut app);
    instantiate.min_voting_period = Some(Duration::Height(10));
    instantiate.max_voting_period = Duration::Height(100);
    let core_addr = instantiate_with_staked_balances_governance(&mut app, instantiate, None);
    let gov_token = query_dao_token(&app, &core_addr);
    let proposal_module = query_single_proposal_module(&app, &core_addr);

    mint_cw20s(&mut app, &gov_token, &core_addr, CREATOR_ADDR, 10_000_000);
    let proposal_id = make_proposal(&mut app, &proposal_module, CREATOR_ADDR, vec![], None);
    vote_on_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        proposal_id,
        Vote::Yes,
    );
    let proposal_response = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal_response.proposal.status, Status::Open);

    app.update_block(|block| block.height += 10);
    let proposal_response = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal_response.proposal.status, Status::Passed);
}

// Setting the min duration the same as the proposal duration just
// means that proposals cant close early.
#[test]
fn test_min_duration_same_as_proposal_duration() {
    let mut app = App::default();
    let mut instantiate = get_default_token_dao_proposal_module_instantiate(&mut app);
    instantiate.min_voting_period = Some(Duration::Height(100));
    instantiate.max_voting_period = Duration::Height(100);
    let core_addr = instantiate_with_staked_balances_governance(
        &mut app,
        instantiate,
        Some(vec![
            Cw20Coin {
                address: "ekez".to_string(),
                amount: Uint128::new(10),
            },
            Cw20Coin {
                address: "whale".to_string(),
                amount: Uint128::new(90),
            },
        ]),
    );
    let gov_token = query_dao_token(&app, &core_addr);
    let proposal_module = query_single_proposal_module(&app, &core_addr);

    mint_cw20s(&mut app, &gov_token, &core_addr, "ekez", 10_000_000);
    let proposal_id = make_proposal(&mut app, &proposal_module, "ekez", vec![], None);

    // Whale votes yes. Normally the proposal would just pass and ekez
    // would be out of luck.
    vote_on_proposal(&mut app, &proposal_module, "whale", proposal_id, Vote::Yes);
    vote_on_proposal(&mut app, &proposal_module, "ekez", proposal_id, Vote::No);

    app.update_block(|b| b.height += 100);
    let proposal_response = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal_response.proposal.status, Status::Passed);
}

#[test]
fn test_revoting_playthrough() {
    let mut app = App::default();
    let mut instantiate = get_default_token_dao_proposal_module_instantiate(&mut app);
    instantiate.allow_revoting = true;
    let core_addr = instantiate_with_staked_balances_governance(&mut app, instantiate, None);
    let gov_token = query_dao_token(&app, &core_addr);
    let proposal_module = query_single_proposal_module(&app, &core_addr);

    mint_cw20s(&mut app, &gov_token, &core_addr, CREATOR_ADDR, 10_000_000);
    let proposal_id = make_proposal(&mut app, &proposal_module, CREATOR_ADDR, vec![], None);

    // Vote and change our minds a couple times.
    vote_on_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        proposal_id,
        Vote::Yes,
    );
    let proposal_response = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal_response.proposal.status, Status::Open);

    vote_on_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        proposal_id,
        Vote::No,
    );
    let proposal_response = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal_response.proposal.status, Status::Open);

    vote_on_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        proposal_id,
        Vote::Yes,
    );
    let proposal_response = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal_response.proposal.status, Status::Open);

    // Can't cast the same vote more than once.
    let err = vote_on_proposal_should_fail(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        proposal_id,
        Vote::Yes,
    );
    assert!(matches!(err, ContractError::AlreadyCast {}));

    // Expire the proposal allowing the votes to be tallied.
    app.update_block(|b| b.time = b.time.plus_seconds(604800));
    let proposal_response = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal_response.proposal.status, Status::Passed);
    execute_proposal(&mut app, &proposal_module, CREATOR_ADDR, proposal_id);

    // Can't vote once the proposal is passed.
    let err = vote_on_proposal_should_fail(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        proposal_id,
        Vote::Yes,
    );
    assert!(matches!(err, ContractError::Expired { .. }));
}

/// Tests that revoting is stored at a per-proposal level. Proposals
/// created while revoting is enabled should not have it disabled if a
/// config change turns if off.
#[test]
fn test_allow_revoting_config_changes() {
    let mut app = App::default();
    let mut instantiate = get_default_token_dao_proposal_module_instantiate(&mut app);
    instantiate.allow_revoting = true;
    let core_addr = instantiate_with_staked_balances_governance(&mut app, instantiate, None);
    let gov_token = query_dao_token(&app, &core_addr);
    let proposal_module = query_single_proposal_module(&app, &core_addr);

    mint_cw20s(&mut app, &gov_token, &core_addr, CREATOR_ADDR, 10_000_000);
    // This proposal should have revoting enable for its entire
    // lifetime.
    let revoting_proposal = make_proposal(&mut app, &proposal_module, CREATOR_ADDR, vec![], None);

    // Update the config of the proposal module to disable revoting.
    app.execute_contract(
        core_addr.clone(),
        proposal_module.clone(),
        &ExecuteMsg::UpdateConfig {
            veto: None,
            threshold: Threshold::ThresholdQuorum {
                quorum: PercentageThreshold::Percent(Decimal::percent(15)),
                threshold: PercentageThreshold::Majority {},
            },
            max_voting_period: Duration::Height(10),
            min_voting_period: None,
            only_members_execute: true,
            // Turn off revoting.
            allow_revoting: false,
            dao: core_addr.to_string(),
            close_proposal_on_execution_failure: false,
        },
        &[],
    )
    .unwrap();

    mint_cw20s(&mut app, &gov_token, &core_addr, CREATOR_ADDR, 10_000_000);
    let no_revoting_proposal =
        make_proposal(&mut app, &proposal_module, CREATOR_ADDR, vec![], None);

    vote_on_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        revoting_proposal,
        Vote::Yes,
    );
    vote_on_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        no_revoting_proposal,
        Vote::Yes,
    );

    // Proposal without revoting should have passed.
    let proposal_resp = query_proposal(&app, &proposal_module, no_revoting_proposal);
    assert_eq!(proposal_resp.proposal.status, Status::Passed);

    // Proposal with revoting should not have passed.
    let proposal_resp = query_proposal(&app, &proposal_module, revoting_proposal);
    assert_eq!(proposal_resp.proposal.status, Status::Open);

    // Can change vote on the revoting proposal.
    vote_on_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        revoting_proposal,
        Vote::No,
    );
    // Expire the revoting proposal and close it.
    app.update_block(|b| b.time = b.time.plus_seconds(604800));
    close_proposal(&mut app, &proposal_module, CREATOR_ADDR, revoting_proposal);
}

/// Tests a simple three of five multisig configuration.
#[test]
fn test_three_of_five_multisig() {
    let mut app = App::default();
    let mut instantiate = get_default_non_token_dao_proposal_module_instantiate(&mut app);
    instantiate.threshold = Threshold::AbsoluteCount {
        threshold: Uint128::new(3),
    };
    instantiate.pre_propose_info = PreProposeInfo::AnyoneMayPropose {};
    let core_addr = instantiate_with_cw4_groups_governance(
        &mut app,
        instantiate,
        Some(vec![
            Cw20Coin {
                address: "one".to_string(),
                amount: Uint128::new(1),
            },
            Cw20Coin {
                address: "two".to_string(),
                amount: Uint128::new(1),
            },
            Cw20Coin {
                address: "three".to_string(),
                amount: Uint128::new(1),
            },
            Cw20Coin {
                address: "four".to_string(),
                amount: Uint128::new(1),
            },
            Cw20Coin {
                address: "five".to_string(),
                amount: Uint128::new(1),
            },
        ]),
    );

    let core_state: dao_interface::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(core_addr, &dao_interface::msg::QueryMsg::DumpState {})
        .unwrap();
    let proposal_module = core_state
        .proposal_modules
        .into_iter()
        .next()
        .unwrap()
        .address;

    let proposal_id = make_proposal(&mut app, &proposal_module, CREATOR_ADDR, vec![], None);

    vote_on_proposal(&mut app, &proposal_module, "one", proposal_id, Vote::Yes);
    vote_on_proposal(&mut app, &proposal_module, "two", proposal_id, Vote::Yes);

    // Make sure it doesn't pass early.
    let proposal: ProposalResponse = query_proposal(&app, &proposal_module, 1);
    assert_eq!(proposal.proposal.status, Status::Open);

    vote_on_proposal(&mut app, &proposal_module, "three", proposal_id, Vote::Yes);

    let proposal: ProposalResponse = query_proposal(&app, &proposal_module, 1);
    assert_eq!(proposal.proposal.status, Status::Passed);

    execute_proposal(&mut app, &proposal_module, "four", proposal_id);

    let proposal: ProposalResponse = query_proposal(&app, &proposal_module, 1);
    assert_eq!(proposal.proposal.status, Status::Executed);

    // Make another proposal which we'll reject.
    let proposal_id = make_proposal(&mut app, &proposal_module, "one", vec![], None);

    vote_on_proposal(&mut app, &proposal_module, "one", proposal_id, Vote::Yes);
    vote_on_proposal(&mut app, &proposal_module, "two", proposal_id, Vote::No);
    vote_on_proposal(&mut app, &proposal_module, "three", proposal_id, Vote::No);
    vote_on_proposal(&mut app, &proposal_module, "four", proposal_id, Vote::No);

    let proposal = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal.proposal.status, Status::Rejected);

    close_proposal(&mut app, &proposal_module, "four", proposal_id);
    let proposal = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal.proposal.status, Status::Closed);
}

#[test]
fn test_three_of_five_multisig_revoting() {
    let mut app = App::default();
    let mut instantiate = get_default_non_token_dao_proposal_module_instantiate(&mut app);
    instantiate.threshold = Threshold::AbsoluteCount {
        threshold: Uint128::new(3),
    };
    instantiate.allow_revoting = true;
    instantiate.pre_propose_info = PreProposeInfo::AnyoneMayPropose {};
    let core_addr = instantiate_with_cw4_groups_governance(
        &mut app,
        instantiate,
        Some(vec![
            Cw20Coin {
                address: "one".to_string(),
                amount: Uint128::new(1),
            },
            Cw20Coin {
                address: "two".to_string(),
                amount: Uint128::new(1),
            },
            Cw20Coin {
                address: "three".to_string(),
                amount: Uint128::new(1),
            },
            Cw20Coin {
                address: "four".to_string(),
                amount: Uint128::new(1),
            },
            Cw20Coin {
                address: "five".to_string(),
                amount: Uint128::new(1),
            },
        ]),
    );

    let core_state: dao_interface::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(core_addr, &dao_interface::msg::QueryMsg::DumpState {})
        .unwrap();
    let proposal_module = core_state
        .proposal_modules
        .into_iter()
        .next()
        .unwrap()
        .address;

    let proposal_id = make_proposal(&mut app, &proposal_module, CREATOR_ADDR, vec![], None);

    vote_on_proposal(&mut app, &proposal_module, "one", proposal_id, Vote::Yes);
    vote_on_proposal(&mut app, &proposal_module, "two", proposal_id, Vote::Yes);

    // Make sure it doesn't pass early.
    let proposal: ProposalResponse = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal.proposal.status, Status::Open);

    vote_on_proposal(&mut app, &proposal_module, "three", proposal_id, Vote::Yes);

    // Revoting is enabled so the proposal is still open.
    let proposal: ProposalResponse = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal.proposal.status, Status::Open);

    // Change our minds.
    vote_on_proposal(&mut app, &proposal_module, "one", proposal_id, Vote::No);
    vote_on_proposal(&mut app, &proposal_module, "two", proposal_id, Vote::No);

    let err =
        vote_on_proposal_should_fail(&mut app, &proposal_module, "two", proposal_id, Vote::No);
    assert!(matches!(err, ContractError::AlreadyCast {}));

    // Expire the revoting proposal and close it.
    app.update_block(|b| b.time = b.time.plus_seconds(604800));
    let proposal: ProposalResponse = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal.proposal.status, Status::Rejected);
}

/// Tests that absolute count style thresholds work with token style
/// voting.
#[test]
fn test_absolute_count_threshold_non_multisig() {
    do_votes_staked_balances(
        vec![
            TestSingleChoiceVote {
                voter: "one".to_string(),
                position: Vote::Yes,
                weight: Uint128::new(10),
                should_execute: ShouldExecute::Yes,
            },
            TestSingleChoiceVote {
                voter: "two".to_string(),
                position: Vote::No,
                weight: Uint128::new(200),
                should_execute: ShouldExecute::Yes,
            },
            TestSingleChoiceVote {
                voter: "three".to_string(),
                position: Vote::Yes,
                weight: Uint128::new(1),
                should_execute: ShouldExecute::Yes,
            },
        ],
        Threshold::AbsoluteCount {
            threshold: Uint128::new(11),
        },
        Status::Passed,
        None,
    );
}

/// Tests that we do not overflow when faced with really high token /
/// vote supply.
#[test]
fn test_large_absolute_count_threshold() {
    do_votes_staked_balances(
        vec![
            TestSingleChoiceVote {
                voter: "two".to_string(),
                position: Vote::No,
                weight: Uint128::new(1),
                should_execute: ShouldExecute::Yes,
            },
            // Can vote up to expiration time.
            TestSingleChoiceVote {
                voter: "one".to_string(),
                position: Vote::Yes,
                weight: Uint128::new(u128::MAX - 1),
                should_execute: ShouldExecute::Yes,
            },
        ],
        Threshold::AbsoluteCount {
            threshold: Uint128::new(u128::MAX),
        },
        Status::Rejected,
        None,
    );

    do_votes_staked_balances(
        vec![
            TestSingleChoiceVote {
                voter: "one".to_string(),
                position: Vote::Yes,
                weight: Uint128::new(u128::MAX - 1),
                should_execute: ShouldExecute::Yes,
            },
            TestSingleChoiceVote {
                voter: "two".to_string(),
                position: Vote::No,
                weight: Uint128::new(1),
                should_execute: ShouldExecute::Yes,
            },
        ],
        Threshold::AbsoluteCount {
            threshold: Uint128::new(u128::MAX),
        },
        Status::Rejected,
        None,
    );
}

#[test]
fn test_proposal_count_initialized_to_zero() {
    let mut app = App::default();
    let pre_propose_info = get_pre_propose_info(&mut app, None, false);
    let core_addr = instantiate_with_staked_balances_governance(
        &mut app,
        InstantiateMsg {
            veto: None,
            threshold: Threshold::ThresholdQuorum {
                threshold: PercentageThreshold::Majority {},
                quorum: PercentageThreshold::Percent(Decimal::percent(10)),
            },
            max_voting_period: Duration::Height(10),
            min_voting_period: None,
            only_members_execute: true,
            allow_revoting: false,
            pre_propose_info,
            close_proposal_on_execution_failure: true,
            delegation_module: None,
        },
        Some(vec![
            Cw20Coin {
                address: "ekez".to_string(),
                amount: Uint128::new(10),
            },
            Cw20Coin {
                address: "innactive".to_string(),
                amount: Uint128::new(90),
            },
        ]),
    );

    let core_state: dao_interface::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(core_addr, &dao_interface::msg::QueryMsg::DumpState {})
        .unwrap();
    let proposal_modules = core_state.proposal_modules;

    assert_eq!(proposal_modules.len(), 1);
    let proposal_single = proposal_modules.into_iter().next().unwrap().address;

    let proposal_count: u64 = app
        .wrap()
        .query_wasm_smart(proposal_single, &QueryMsg::ProposalCount {})
        .unwrap();
    assert_eq!(proposal_count, 0);
}

#[test]
fn test_migrate_from_compatible() {
    let CommonTest {
        mut app,
        core_addr,
        proposal_module,
        gov_token: _,
        proposal_id: _,
    } = setup_test(vec![]);

    let new_code_id = app.store_code(dao_proposal_single_contract());
    let start_config = query_proposal_config(&app, &proposal_module);

    app.execute(
        core_addr,
        CosmosMsg::Wasm(WasmMsg::Migrate {
            contract_addr: proposal_module.to_string(),
            new_code_id,
            msg: to_json_binary(&MigrateMsg::FromCompatible {}).unwrap(),
        }),
    )
    .unwrap();

    let end_config = query_proposal_config(&app, &proposal_module);
    assert_eq!(start_config, end_config);
}

#[test]
pub fn test_migrate_updates_version() {
    let mut deps = mock_dependencies();
    cw2::set_contract_version(&mut deps.storage, "my-contract", "old-version").unwrap();
    migrate(deps.as_mut(), mock_env(), MigrateMsg::FromCompatible {}).unwrap();
    let version = cw2::get_contract_version(&deps.storage).unwrap();
    assert_eq!(version.version, CONTRACT_VERSION);
    assert_eq!(version.contract, CONTRACT_NAME);
}

// //// TODO test migrate
// /// Instantiates a DAO with a v1 proposal module and then migrates it
// /// to v2.
// #[test]
// fn test_migrate_from_v1() {
//     use cw_proposal_single_v1 as v1;
//     use dao_pre_propose_single as cppbps;

//     let mut app = App::default();
//     let v1_proposal_single_code = app.store_code(v1_proposal_single_contract());

//     let instantiate = v1::msg::InstantiateMsg {
//         threshold: voting_v1::Threshold::AbsolutePercentage {
//             percentage: voting_v1::PercentageThreshold::Majority {},
//         },
//         max_voting_period: cw_utils_v1::Duration::Height(6),
//         min_voting_period: None,
//         only_members_execute: false,
//         allow_revoting: false,
//         deposit_info: Some(v1::msg::DepositInfo {
//             token: v1::msg::DepositToken::VotingModuleToken {
//                 token_type: VotingModuleTokenType::Cw20,
//             },
//             deposit: Uint128::new(1),
//             refund_failed_proposals: true,
//         }),
//     };

//     let initial_balances = vec![Cw20Coin {
//         amount: Uint128::new(100),
//         address: CREATOR_ADDR.to_string(),
//     }];

//     let cw20_id = app.store_code(cw20_base_contract());
//     let cw20_stake_id = app.store_code(cw20_stake_contract());
//     let staked_balances_voting_id = app.store_code(dao_voting_cw20_staked_contract());
//     let core_contract_id = app.store_code(dao_dao_core_contract());

// let instantiate_core = dao_interface::msg::InstantiateMsg {
//     admin: None,
//     name: "DAO DAO".to_string(),
//     description: "A DAO that builds DAOs".to_string(),
//     image_url: None,
//     dao_uri: None,
//     automatically_add_cw20s: true,
//     automatically_add_cw721s: false,
//     voting_module_instantiate_info: ModuleInstantiateInfo {
//         code_id: staked_balances_voting_id,
//         msg: to_json_binary(&dao_voting_cw20_staked::msg::InstantiateMsg {
//             active_threshold: None,
//             token_info: dao_voting_cw20_staked::msg::TokenInfo::New {
//                 code_id: cw20_id,
//                 label: "DAO DAO governance token.".to_string(),
//                 name: "DAO DAO".to_string(),
//                 symbol: "DAO".to_string(),
//                 decimals: 6,
//                 initial_balances: initial_balances.clone(),
//                 marketing: None,
//                 staking_code_id: cw20_stake_id,
//                 unstaking_duration: Some(Duration::Height(6)),
//                 initial_dao_balance: None,
//             },
//         })
//         .unwrap(),
//         admin: None,
//         funds: vec![],
//         label: "DAO DAO voting module".to_string(),
//     },
//     proposal_modules_instantiate_info: vec![ModuleInstantiateInfo {
//         code_id: v1_proposal_single_code,
//         msg: to_json_binary(&instantiate).unwrap(),
//         admin: Some(Admin::CoreModule {}),
//         funds: vec![],
//         label: "DAO DAO governance module.".to_string(),
//     }],
//     initial_items: None,
// };

//     let core_addr = app
//         .instantiate_contract(
//             core_contract_id,
//             Addr::unchecked(CREATOR_ADDR),
//             &instantiate_core,
//             &[],
//             "DAO DAO",
//             None,
//         )
//         .unwrap();

//     let core_state: dao_interface::query::DumpStateResponse = app
//         .wrap()
//         .query_wasm_smart(
//             core_addr.clone(),
//             &dao_interface::msg::QueryMsg::DumpState {},
//         )
//         .unwrap();
//     let voting_module = core_state.voting_module;

//     let staking_contract: Addr = app
//         .wrap()
//         .query_wasm_smart(
//             voting_module.clone(),
//             &dao_voting_cw20_staked::msg::QueryMsg::StakingContract {},
//         )
//         .unwrap();
//     let token_contract: Addr = app
//         .wrap()
//         .query_wasm_smart(
//             voting_module,
//             &dao_interface::voting::Query::TokenContract {},
//         )
//         .unwrap();

//     // Stake all the initial balances.
//     for Cw20Coin { address, amount } in initial_balances {
//         app.execute_contract(
//             Addr::unchecked(address),
//             token_contract.clone(),
//             &cw20::Cw20ExecuteMsg::Send {
//                 contract: staking_contract.to_string(),
//                 amount,
//                 msg: to_json_binary(&cw20_stake::msg::ReceiveMsg::Stake {}).unwrap(),
//             },
//             &[],
//         )
//         .unwrap();
//     }

//     // Update the block so that those staked balances appear.
//     app.update_block(|block| block.height += 1);

//     let proposal_module = query_single_proposal_module(&app, &core_addr);

//     // Make a proposal so we can test that migration doesn't work with
//     // open proposals that have deposits.
//     mint_cw20s(&mut app, &token_contract, &core_addr, CREATOR_ADDR, 1);
//     app.execute_contract(
//         Addr::unchecked(CREATOR_ADDR),
//         token_contract.clone(),
//         &cw20::Cw20ExecuteMsg::IncreaseAllowance {
//             spender: proposal_module.to_string(),
//             amount: Uint128::new(1),
//             expires: None,
//         },
//         &[],
//     )
//     .unwrap();
//     app.execute_contract(
//         Addr::unchecked(CREATOR_ADDR),
//         proposal_module.clone(),
//         &v1::msg::ExecuteMsg::Propose {
//             title: "title".to_string(),
//             description: "description".to_string(),
//             msgs: vec![],
//         },
//         &[],
//     )
//     .unwrap();

//     let v2_proposal_single = app.store_code(dao_proposal_single_contract());
//     let pre_propose_single = app.store_code(dao_pre_propose_approval_single_contract());

//     // Attempt to migrate. This will fail as there is a pending
//     // proposal.
//     let migrate_msg = MigrateMsg::FromV2 { timelock: None };
//     let err: ContractError = app
//         .execute(
//             core_addr.clone(),
//             CosmosMsg::Wasm(WasmMsg::Migrate {
//                 contract_addr: proposal_module.to_string(),
//                 new_code_id: v2_proposal_single,
//                 msg: to_json_binary(&migrate_msg).unwrap(),
//             }),
//         )
//         .unwrap_err()
//         .downcast()
//         .unwrap();
//     assert!(matches!(err, ContractError::PendingProposals {}));

//     // Vote on and close the pending proposal.
//     vote_on_proposal(&mut app, &proposal_module, CREATOR_ADDR, 1, Vote::No);
//     close_proposal(&mut app, &proposal_module, CREATOR_ADDR, 1);

//     // Now we can migrate!
//     app.execute(
//         core_addr.clone(),
//         CosmosMsg::Wasm(WasmMsg::Migrate {
//             contract_addr: proposal_module.to_string(),
//             new_code_id: v2_proposal_single,
//             msg: to_json_binary(&migrate_msg).unwrap(),
//         }),
//     )
//     .unwrap();

//     let new_config = query_proposal_config(&app, &proposal_module);
//     assert_eq!(
//         new_config,
//         Config {
//             timelock: None,
//             threshold: Threshold::AbsolutePercentage {
//                 percentage: PercentageThreshold::Majority {}
//             },
//             max_voting_period: Duration::Height(6),
//             min_voting_period: None,
//             only_members_execute: false,
//             allow_revoting: false,
//             dao: core_addr.clone(),
//             close_proposal_on_execution_failure: true,
//         }
//     );

//     // We can not migrate more than once.
//     let err: ContractError = app
//         .execute(
//             core_addr.clone(),
//             CosmosMsg::Wasm(WasmMsg::Migrate {
//                 contract_addr: proposal_module.to_string(),
//                 new_code_id: v2_proposal_single,
//                 msg: to_json_binary(&migrate_msg).unwrap(),
//             }),
//         )
//         .unwrap_err()
//         .downcast()
//         .unwrap();
//     assert!(matches!(err, ContractError::AlreadyMigrated {}));

//     // Make sure we can still query for ballots (rationale works post
//     // migration).
//     let vote = query_vote(&app, &proposal_module, CREATOR_ADDR, 1);
//     assert_eq!(
//         vote.vote.unwrap(),
//         VoteInfo {
//             voter: Addr::unchecked(CREATOR_ADDR),
//             vote: Vote::No,
//             power: Uint128::new(100),
//             rationale: None
//         }
//     );

//     let proposal_creation_policy = query_creation_policy(&app, &proposal_module);

//     // Check that a new creation policy has been birthed.
//     let pre_propose = match proposal_creation_policy {
//         ProposalCreationPolicy::Anyone {} => panic!("expected a pre-propose module"),
//         ProposalCreationPolicy::Module { addr } => addr,
//     };
//     let pre_propose_config = query_pre_proposal_single_config(&app, &pre_propose);
//     assert_eq!(
//         pre_propose_config,
//         cppbps::Config {
//             open_proposal_submission: false,
//             deposit_info: Some(CheckedDepositInfo {
//                 denom: CheckedDenom::Cw20(token_contract.clone()),
//                 amount: Uint128::new(1),
//                 refund_policy: dao_voting::deposit::DepositRefundPolicy::OnlyPassed,
//             })
//         }
//     );

//     // Make sure we can still make a proposal and vote on it.
//     mint_cw20s(&mut app, &token_contract, &core_addr, CREATOR_ADDR, 1);
//     let proposal_id = make_proposal(&mut app, &proposal_module, CREATOR_ADDR, vec![], None);
//     vote_on_proposal(
//         &mut app,
//         &proposal_module,
//         CREATOR_ADDR,
//         proposal_id,
//         Vote::Yes,
//     );
//     execute_proposal(&mut app, &proposal_module, CREATOR_ADDR, proposal_id);
//     let proposal = query_proposal(&app, &proposal_module, proposal_id);
//     assert_eq!(proposal.proposal.status, Status::Executed);
// }

// - Make a proposal that will fail to execute.
// - Verify that it goes to execution failed and that proposal
//   deposits are returned once and not on closing.
// - Make the same proposal again.
// - Update the config to disable close on execution failure.
// - Make sure that proposal doesn't close on execution (this config
//   feature gets applied retroactively).
#[test]
fn test_execution_failed() {
    let CommonTest {
        mut app,
        core_addr,
        proposal_module,
        gov_token,
        proposal_id,
    } = setup_test(vec![BankMsg::Send {
        to_address: "ekez".to_string(),
        amount: coins(10, "ujuno"),
    }
    .into()]);

    vote_on_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        proposal_id,
        Vote::Yes,
    );
    execute_proposal(&mut app, &proposal_module, CREATOR_ADDR, proposal_id);

    let proposal = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal.proposal.status, Status::ExecutionFailed);

    // Make sure the deposit was returned.
    let balance = query_balance_cw20(&app, &gov_token, CREATOR_ADDR);
    assert_eq!(balance, Uint128::new(10_000_000));

    // ExecutionFailed is an end state.
    let err = close_proposal_should_fail(&mut app, &proposal_module, CREATOR_ADDR, proposal_id);
    assert!(matches!(err, ContractError::WrongCloseStatus {}));

    let proposal_id = make_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        vec![BankMsg::Send {
            to_address: "ekez".to_string(),
            amount: coins(10, "ujuno"),
        }
        .into()],
        None,
    );

    let config = query_proposal_config(&app, &proposal_module);

    // Disable execution failing proposals.
    app.execute_contract(
        core_addr,
        proposal_module.clone(),
        &ExecuteMsg::UpdateConfig {
            veto: None,
            threshold: config.threshold,
            max_voting_period: config.max_voting_period,
            min_voting_period: config.min_voting_period,
            only_members_execute: config.only_members_execute,
            allow_revoting: config.allow_revoting,
            dao: config.dao.into_string(),
            // Disable.
            close_proposal_on_execution_failure: false,
        },
        &[],
    )
    .unwrap();

    vote_on_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        proposal_id,
        Vote::Yes,
    );
    let err: StdError = app
        .execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            proposal_module.clone(),
            &ExecuteMsg::Execute { proposal_id },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert!(matches!(err, StdError::Overflow { .. }));

    // Even though this proposal was created before the config change
    // was made it still gets retroactively applied.
    let proposal = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal.proposal.status, Status::Passed);

    // This proposal's deposit should not have been returned. It will
    // not be returnable until this is executed, or close on execution
    // is re-enabled.
    let balance = query_balance_cw20(&app, &gov_token, CREATOR_ADDR);
    assert_eq!(balance, Uint128::zero());
}

#[test]
fn test_reply_proposal_mock() {
    use crate::contract::reply;
    use crate::state::PROPOSALS;

    let mut deps = mock_dependencies();
    let env = mock_env();

    let m_proposal_id = mask_proposal_execution_proposal_id(1);
    PROPOSALS
        .save(
            deps.as_mut().storage,
            1,
            &SingleChoiceProposal {
                title: "A simple text proposal".to_string(),
                description: "This is a simple text proposal".to_string(),
                proposer: Addr::unchecked(CREATOR_ADDR),
                start_height: env.block.height,
                expiration: cw_utils::Duration::Height(6).after(&env.block),
                min_voting_period: None,
                threshold: Threshold::AbsolutePercentage {
                    percentage: PercentageThreshold::Majority {},
                },
                allow_revoting: false,
                total_power: Uint128::new(100_000_000),
                msgs: vec![],
                status: Status::Open,
                veto: None,
                votes: Votes::zero(),
                individual_votes: Votes::zero(),
                delegation_module: None,
            },
        )
        .unwrap();

    // PROPOSALS
    let reply_msg = Reply {
        id: m_proposal_id,
        result: SubMsgResult::Err("error_msg".to_string()),
    };
    let res = reply(deps.as_mut(), env, reply_msg).unwrap();
    assert_eq!(
        res.attributes[0],
        Attribute {
            key: "proposal_execution_failed".to_string(),
            value: 1.to_string()
        }
    );

    let prop = PROPOSALS.load(deps.as_mut().storage, 1).unwrap();
    assert_eq!(prop.status, Status::ExecutionFailed);
}

#[test]
fn test_proposal_too_large() {
    let mut app = App::default();
    let mut instantiate = get_default_token_dao_proposal_module_instantiate(&mut app);
    instantiate.pre_propose_info = PreProposeInfo::AnyoneMayPropose {};
    let core_addr = instantiate_with_staked_balances_governance(&mut app, instantiate, None);
    let proposal_module = query_single_proposal_module(&app, &core_addr);

    let err = app
        .execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            proposal_module,
            &ExecuteMsg::Propose(ProposeMsg {
                title: "".to_string(),
                description: "a".repeat(MAX_PROPOSAL_SIZE as usize),
                msgs: vec![],
                proposer: None,
                vote: None,
            }),
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert!(matches!(
        err,
        ContractError::ProposalTooLarge {
            size: _,
            max: MAX_PROPOSAL_SIZE
        }
    ))
}

#[test]
fn test_vote_not_registered() {
    let CommonTest {
        mut app,
        core_addr: _,
        proposal_module,
        gov_token: _,
        proposal_id,
    } = setup_test(vec![]);

    let err =
        vote_on_proposal_should_fail(&mut app, &proposal_module, "ekez", proposal_id, Vote::Yes);
    assert!(matches!(err, ContractError::NotRegistered {}))
}

#[test]
fn test_proposal_creation_permissions() {
    let CommonTest {
        mut app,
        core_addr,
        proposal_module,
        gov_token: _,
        proposal_id: _,
    } = setup_test(vec![]);

    // Non pre-propose may not propose.
    let err = app
        .execute_contract(
            Addr::unchecked("notprepropose"),
            proposal_module.clone(),
            &ExecuteMsg::Propose(ProposeMsg {
                title: "title".to_string(),
                description: "description".to_string(),
                msgs: vec![],
                proposer: None,
                vote: None,
            }),
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert!(matches!(err, ContractError::Unauthorized {}));

    let proposal_creation_policy = query_creation_policy(&app, &proposal_module);
    let pre_propose = match proposal_creation_policy {
        ProposalCreationPolicy::Anyone {} => panic!("expected a pre-propose module"),
        ProposalCreationPolicy::Module { addr } => addr,
    };

    // Proposer may not be none when a pre-propose module is making
    // the proposal.
    let err = app
        .execute_contract(
            pre_propose,
            proposal_module.clone(),
            &ExecuteMsg::Propose(ProposeMsg {
                title: "title".to_string(),
                description: "description".to_string(),
                msgs: vec![],
                proposer: None,
                vote: None,
            }),
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert!(matches!(err, ContractError::InvalidProposer {}));

    // Allow anyone to propose.
    app.execute_contract(
        core_addr,
        proposal_module.clone(),
        &ExecuteMsg::UpdatePreProposeInfo {
            info: PreProposeInfo::AnyoneMayPropose {},
        },
        &[],
    )
    .unwrap();

    // Proposer must be None when non pre-propose module is making the
    // proposal.
    let err = app
        .execute_contract(
            Addr::unchecked("ekez"),
            proposal_module.clone(),
            &ExecuteMsg::Propose(ProposeMsg {
                title: "title".to_string(),
                description: "description".to_string(),
                msgs: vec![],
                proposer: Some("ekez".to_string()),
                vote: None,
            }),
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert!(matches!(err, ContractError::InvalidProposer {}));

    // Works normally.
    let proposal_id = make_proposal(&mut app, &proposal_module, "ekez", vec![], None);
    let proposal = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal.proposal.proposer, Addr::unchecked("ekez"));
    vote_on_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        proposal_id,
        Vote::No,
    );
    close_proposal(&mut app, &proposal_module, CREATOR_ADDR, proposal_id);
}

#[test]
fn test_reply_hooks_mock() {
    use crate::contract::reply;
    use crate::state::{CREATION_POLICY, PROPOSAL_HOOKS, VOTE_HOOKS};

    let mut deps = mock_dependencies();
    let env = mock_env();

    // Add a proposal hook and remove it
    let m_proposal_hook_idx = mask_proposal_hook_index(0);
    PROPOSAL_HOOKS
        .add_hook(deps.as_mut().storage, Addr::unchecked(CREATOR_ADDR))
        .unwrap();

    let reply_msg = Reply {
        id: m_proposal_hook_idx,
        result: SubMsgResult::Err("error_msg".to_string()),
    };

    let res = reply(deps.as_mut(), env.clone(), reply_msg).unwrap();
    assert_eq!(
        res.attributes[0],
        Attribute {
            key: "removed_proposal_hook".to_string(),
            value: format! {"{CREATOR_ADDR}:{}", 0}
        }
    );

    // Reply needs a creation policy in state.
    CREATION_POLICY
        .save(
            deps.as_mut().storage,
            &ProposalCreationPolicy::Module {
                addr: Addr::unchecked("ekez"),
            },
        )
        .unwrap();

    let prepropose_reply_msg = Reply {
        id: failed_pre_propose_module_hook_id(),
        result: SubMsgResult::Err("error_msg".to_string()),
    };

    // Remove the pre-propose module as part of a reply.
    let res = reply(deps.as_mut(), env.clone(), prepropose_reply_msg.clone()).unwrap();
    assert_eq!(
        res.attributes[0],
        Attribute {
            key: "failed_prepropose_hook".to_string(),
            value: "ekez".into()
        }
    );

    // Do it again. This time, there is no module so this should error.
    let _id = failed_pre_propose_module_hook_id();
    let err = reply(deps.as_mut(), env.clone(), prepropose_reply_msg).unwrap_err();
    assert!(matches!(
        err,
        crate::ContractError::InvalidReplyID { id: _ }
    ));

    // Check that we fail open.
    let status = CREATION_POLICY.load(deps.as_ref().storage).unwrap();
    assert!(matches!(status, ProposalCreationPolicy::Anyone {}));

    // Vote hook
    let m_vote_hook_idx = mask_vote_hook_index(0);
    VOTE_HOOKS
        .add_hook(deps.as_mut().storage, Addr::unchecked(CREATOR_ADDR))
        .unwrap();

    let reply_msg = Reply {
        id: m_vote_hook_idx,
        result: SubMsgResult::Err("error_msg".to_string()),
    };
    let res = reply(deps.as_mut(), env, reply_msg).unwrap();
    assert_eq!(
        res.attributes[0],
        Attribute {
            key: "removed_vote_hook".to_string(),
            value: format! {"{CREATOR_ADDR}:{}", 0}
        }
    );
}

#[test]
fn test_query_info() {
    let CommonTest {
        app,
        core_addr: _,
        proposal_module,
        gov_token: _,
        proposal_id: _,
    } = setup_test(vec![]);
    let info: InfoResponse = app
        .wrap()
        .query_wasm_smart(proposal_module, &QueryMsg::Info {})
        .unwrap();
    assert_eq!(
        info,
        InfoResponse {
            info: ContractVersion {
                contract: CONTRACT_NAME.to_string(),
                version: CONTRACT_VERSION.to_string()
            }
        }
    )
}

// Make a little multisig and test that queries to list votes work as
// expected.
#[test]
fn test_query_list_votes() {
    let mut app = App::default();
    let mut instantiate = get_default_non_token_dao_proposal_module_instantiate(&mut app);
    instantiate.threshold = Threshold::AbsoluteCount {
        threshold: Uint128::new(3),
    };
    instantiate.pre_propose_info = PreProposeInfo::AnyoneMayPropose {};
    let core_addr = instantiate_with_cw4_groups_governance(
        &mut app,
        instantiate,
        Some(vec![
            Cw20Coin {
                address: "one".to_string(),
                amount: Uint128::new(1),
            },
            Cw20Coin {
                address: "two".to_string(),
                amount: Uint128::new(1),
            },
            Cw20Coin {
                address: "three".to_string(),
                amount: Uint128::new(1),
            },
            Cw20Coin {
                address: "four".to_string(),
                amount: Uint128::new(1),
            },
            Cw20Coin {
                address: "five".to_string(),
                amount: Uint128::new(1),
            },
        ]),
    );
    let proposal_module = query_single_proposal_module(&app, &core_addr);
    let proposal_id = make_proposal(&mut app, &proposal_module, "one", vec![], None);

    let votes = query_list_votes(&app, &proposal_module, proposal_id, None, None);
    assert_eq!(votes.votes, vec![]);

    vote_on_proposal(&mut app, &proposal_module, "two", proposal_id, Vote::No);
    vote_on_proposal(&mut app, &proposal_module, "three", proposal_id, Vote::No);
    vote_on_proposal(&mut app, &proposal_module, "one", proposal_id, Vote::Yes);
    vote_on_proposal(&mut app, &proposal_module, "four", proposal_id, Vote::Yes);
    vote_on_proposal(&mut app, &proposal_module, "five", proposal_id, Vote::Yes);

    let votes = query_list_votes(&app, &proposal_module, proposal_id, None, None);
    assert_eq!(
        votes.votes,
        vec![
            VoteInfo {
                rationale: None,
                voter: Addr::unchecked("five"),
                vote: Vote::Yes,
                power: Uint128::new(1)
            },
            VoteInfo {
                rationale: None,
                voter: Addr::unchecked("four"),
                vote: Vote::Yes,
                power: Uint128::new(1)
            },
            VoteInfo {
                rationale: None,
                voter: Addr::unchecked("one"),
                vote: Vote::Yes,
                power: Uint128::new(1)
            },
            VoteInfo {
                rationale: None,
                voter: Addr::unchecked("three"),
                vote: Vote::No,
                power: Uint128::new(1)
            },
            VoteInfo {
                rationale: None,
                voter: Addr::unchecked("two"),
                vote: Vote::No,
                power: Uint128::new(1)
            }
        ]
    );

    let votes = query_list_votes(
        &app,
        &proposal_module,
        proposal_id,
        Some("four".to_string()),
        Some(2),
    );
    assert_eq!(
        votes.votes,
        vec![
            VoteInfo {
                rationale: None,
                voter: Addr::unchecked("one"),
                vote: Vote::Yes,
                power: Uint128::new(1)
            },
            VoteInfo {
                rationale: None,
                voter: Addr::unchecked("three"),
                vote: Vote::No,
                power: Uint128::new(1)
            },
        ]
    );
}

#[test]
fn test_update_pre_propose_module() {
    let CommonTest {
        mut app,
        core_addr,
        proposal_module,
        gov_token,
        proposal_id: pre_update_proposal_id,
    } = setup_test(vec![]);

    // Store the address of the pre-propose module that we start with
    // so we can execute withdraw on it later.
    let proposal_creation_policy = query_creation_policy(&app, &proposal_module);
    let pre_propose_start = match proposal_creation_policy {
        ProposalCreationPolicy::Anyone {} => panic!("expected a pre-propose module"),
        ProposalCreationPolicy::Module { addr } => addr,
    };

    let pre_propose_id = app.store_code(dao_pre_propose_single_contract());

    // Make a proposal to switch to a new pre-propose moudle.
    mint_cw20s(&mut app, &gov_token, &core_addr, CREATOR_ADDR, 10_000_000);
    let proposal_id = make_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        vec![WasmMsg::Execute {
            contract_addr: proposal_module.to_string(),
            msg: to_json_binary(&ExecuteMsg::UpdatePreProposeInfo {
                info: PreProposeInfo::ModuleMayPropose {
                    info: ModuleInstantiateInfo {
                        code_id: pre_propose_id,
                        msg: to_json_binary(&dao_pre_propose_single::InstantiateMsg {
                            deposit_info: Some(UncheckedDepositInfo {
                                denom: dao_voting::deposit::DepositToken::VotingModuleToken {
                                    token_type: VotingModuleTokenType::Cw20,
                                },
                                amount: Uint128::new(1),
                                refund_policy: dao_voting::deposit::DepositRefundPolicy::OnlyPassed,
                            }),
                            submission_policy: PreProposeSubmissionPolicy::Specific {
                                dao_members: true,
                                allowlist: vec![],
                                denylist: vec![],
                            },
                            extension: Empty::default(),
                        })
                        .unwrap(),
                        admin: Some(Admin::CoreModule {}),
                        funds: vec![],
                        label: "new pre-propose module".to_string(),
                    },
                },
            })
            .unwrap(),
            funds: vec![],
        }
        .into()],
        None,
    );

    vote_on_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        proposal_id,
        Vote::Yes,
    );
    execute_proposal(&mut app, &proposal_module, CREATOR_ADDR, proposal_id);

    // Check that a new creation policy has been birthed.
    let proposal_creation_policy = query_creation_policy(&app, &proposal_module);
    let pre_propose = match proposal_creation_policy {
        ProposalCreationPolicy::Anyone {} => panic!("expected a pre-propose module"),
        ProposalCreationPolicy::Module { addr } => addr,
    };

    // Check that the admin has been set to the DAO properly.
    let info: ContractInfoResponse = app
        .wrap()
        .query(&cosmwasm_std::QueryRequest::Wasm(WasmQuery::ContractInfo {
            contract_addr: pre_propose.to_string(),
        }))
        .unwrap();
    assert_eq!(info.admin, Some(core_addr.to_string()));

    let pre_propose_config = query_pre_proposal_single_config(&app, &pre_propose);
    assert_eq!(
        pre_propose_config,
        dao_pre_propose_single::Config {
            deposit_info: Some(CheckedDepositInfo {
                denom: CheckedDenom::Cw20(gov_token.clone()),
                amount: Uint128::new(1),
                refund_policy: dao_voting::deposit::DepositRefundPolicy::OnlyPassed,
            }),
            submission_policy: PreProposeSubmissionPolicy::Specific {
                dao_members: true,
                allowlist: vec![],
                denylist: vec![]
            },
        }
    );

    // Make a new proposal with this new module installed.
    make_proposal(&mut app, &proposal_module, CREATOR_ADDR, vec![], None);
    // Check that the deposit was withdrawn.
    let balance = query_balance_cw20(&app, gov_token.as_str(), CREATOR_ADDR);
    assert_eq!(balance, Uint128::new(9_999_999));

    // Vote on and execute the proposal created with the old
    // module. This should work fine, but the deposit will not be
    // returned as that module is no longer receiving hook messages.
    vote_on_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        pre_update_proposal_id,
        Vote::Yes,
    );
    execute_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        pre_update_proposal_id,
    );

    // Deposit should not have been returned.
    let balance = query_balance_cw20(&app, gov_token.as_str(), CREATOR_ADDR);
    assert_eq!(balance, Uint128::new(9_999_999));

    // Withdraw from the old pre-propose module.
    let proposal_id = make_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        vec![WasmMsg::Execute {
            contract_addr: pre_propose_start.into_string(),
            msg: to_json_binary(&dao_pre_propose_single::ExecuteMsg::Withdraw { denom: None })
                .unwrap(),
            funds: vec![],
        }
        .into()],
        None,
    );
    vote_on_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        proposal_id,
        Vote::Yes,
    );
    execute_proposal(&mut app, &proposal_module, CREATOR_ADDR, proposal_id);

    // Make sure the left over deposit was returned to the DAO.
    let balance = query_balance_cw20(&app, gov_token.as_str(), core_addr.as_str());
    assert_eq!(balance, Uint128::new(10_000_000));
}

/// DAO should be admin of the pre-propose contract despite the fact
/// that the proposal module instantiates it.
#[test]
fn test_pre_propose_admin_is_dao() {
    let CommonTest {
        app,
        core_addr,
        proposal_module,
        gov_token: _,
        proposal_id: _,
    } = setup_test(vec![]);

    let proposal_creation_policy = query_creation_policy(&app, &proposal_module);

    // Check that a new creation policy has been birthed.
    let pre_propose = match proposal_creation_policy {
        ProposalCreationPolicy::Anyone {} => panic!("expected a pre-propose module"),
        ProposalCreationPolicy::Module { addr } => addr,
    };

    let info: ContractInfoResponse = app
        .wrap()
        .query(&cosmwasm_std::QueryRequest::Wasm(WasmQuery::ContractInfo {
            contract_addr: pre_propose.into_string(),
        }))
        .unwrap();
    assert_eq!(info.admin, Some(core_addr.into_string()));
}

// I can add a rationale to my vote. My rational is queryable when
// listing votes. I can later change my rationale.
#[test]
fn test_rationale() {
    let CommonTest {
        mut app,
        proposal_module,
        proposal_id,
        ..
    } = setup_test(vec![]);

    let rationale = Some("i support dog charities".to_string());

    vote_on_proposal_with_rationale(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        proposal_id,
        Vote::Yes,
        rationale.clone(),
    );

    let vote = query_vote(&app, &proposal_module, CREATOR_ADDR, proposal_id);
    assert_eq!(vote.vote.unwrap().rationale, rationale);

    let rationale =
        Some("i did not realize that dog charity was gambling with customer funds".to_string());

    update_rationale(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        proposal_id,
        rationale.clone(),
    );

    let vote = query_vote(&app, &proposal_module, CREATOR_ADDR, proposal_id);
    assert_eq!(vote.vote.unwrap().rationale, rationale);
}

// Revoting should override any previous rationale. If no new
// rationalle is provided, the old one will be wiped regardless.
#[test]
fn test_rational_clobbered_on_revote() {
    let mut app = App::default();
    let mut instantiate = get_default_token_dao_proposal_module_instantiate(&mut app);
    instantiate.allow_revoting = true;
    let core_addr = instantiate_with_staked_balances_governance(&mut app, instantiate, None);
    let gov_token = query_dao_token(&app, &core_addr);
    let proposal_module = query_single_proposal_module(&app, &core_addr);

    mint_cw20s(&mut app, &gov_token, &core_addr, CREATOR_ADDR, 10_000_000);
    let proposal_id = make_proposal(&mut app, &proposal_module, CREATOR_ADDR, vec![], None);

    let rationale = Some("to_string".to_string());

    vote_on_proposal_with_rationale(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        proposal_id,
        Vote::Yes,
        rationale.clone(),
    );

    let vote = query_vote(&app, &proposal_module, CREATOR_ADDR, proposal_id);
    assert_eq!(vote.vote.unwrap().rationale, rationale);

    let rationale = None;

    // revote and clobber.
    vote_on_proposal_with_rationale(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        proposal_id,
        Vote::No,
        None,
    );

    let vote = query_vote(&app, &proposal_module, CREATOR_ADDR, proposal_id);
    assert_eq!(vote.vote.unwrap().rationale, rationale);
}

// Casting votes is only allowed within the proposal expiration timeframe
#[test]
pub fn test_not_allow_voting_on_expired_proposal() {
    let CommonTest {
        mut app,
        core_addr: _,
        proposal_module,
        gov_token: _,
        proposal_id,
    } = setup_test(vec![]);

    // expire the proposal
    app.update_block(|b| b.time = b.time.plus_seconds(604800));
    let proposal = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal.proposal.status, Status::Rejected);
    assert_eq!(proposal.proposal.votes.yes, Uint128::zero());

    // attempt to vote past the expiration date
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            proposal_module.clone(),
            &ExecuteMsg::Vote {
                proposal_id,
                vote: Vote::Yes,
                rationale: None,
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    // assert the vote got rejected and did not count
    // towards the votes
    let proposal = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal.proposal.status, Status::Rejected);
    assert_eq!(proposal.proposal.votes.yes, Uint128::zero());
    assert!(matches!(err, ContractError::Expired { id: _proposal_id }));
}

#[test]
fn test_proposal_count_goes_up() {
    let CommonTest {
        mut app,
        proposal_module,
        gov_token,
        core_addr,
        ..
    } = setup_test(vec![]);

    let next = query_next_proposal_id(&app, &proposal_module);
    assert_eq!(next, 2);

    mint_cw20s(&mut app, &gov_token, &core_addr, CREATOR_ADDR, 10_000_000);
    make_proposal(&mut app, &proposal_module, CREATOR_ADDR, vec![], None);

    let next = query_next_proposal_id(&app, &proposal_module);
    assert_eq!(next, 3);
}
