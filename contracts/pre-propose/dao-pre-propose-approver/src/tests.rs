use cosmwasm_std::{coins, from_json, to_json_binary, Addr, Coin, Uint128};
use cw2::ContractVersion;
use cw20::Cw20Coin;
use cw_denom::UncheckedDenom;
use cw_multi_test::{App, BankSudo, Executor};
use dao_interface::proposal::InfoResponse;
use dao_voting::pre_propose::{PreProposeSubmissionPolicy, PreProposeSubmissionPolicyError};
use dps::query::{ProposalListResponse, ProposalResponse};

use dao_interface::state::ProposalModule;
use dao_interface::state::{Admin, ModuleInstantiateInfo};
use dao_pre_propose_approval_single::{
    msg::{
        ExecuteExt, ExecuteMsg, InstantiateExt, InstantiateMsg, ProposeMessage, QueryExt, QueryMsg,
    },
    state::Proposal,
};
use dao_pre_propose_base::{error::PreProposeError, msg::DepositInfoResponse, state::Config};
use dao_proposal_single as dps;
use dao_testing::contracts::{
    cw20_base_contract, dao_pre_propose_approval_single_contract,
    dao_pre_propose_approver_contract, dao_proposal_single_contract,
};
use dao_testing::helpers::instantiate_with_cw4_groups_governance;
use dao_voting::{
    deposit::{CheckedDepositInfo, DepositRefundPolicy, DepositToken, UncheckedDepositInfo},
    pre_propose::{PreProposeInfo, ProposalCreationPolicy},
    status::Status,
    threshold::{PercentageThreshold, Threshold},
    voting::Vote,
};

use crate::contract::{CONTRACT_NAME, CONTRACT_VERSION};
use crate::msg::InstantiateMsg as ApproverInstantiateMsg;
use crate::msg::{
    ExecuteExt as ApproverExecuteExt, ExecuteMsg as ApproverExecuteMsg,
    QueryExt as ApproverQueryExt, QueryMsg as ApproverQueryMsg,
};

// The approver dao contract is the 6th contract instantiated
const APPROVER: &str = "contract6";

fn get_proposal_module_approval_single_instantiate(
    app: &mut App,
    deposit_info: Option<UncheckedDepositInfo>,
    open_proposal_submission: bool,
) -> dps::msg::InstantiateMsg {
    let pre_propose_id = app.store_code(dao_pre_propose_approval_single_contract());

    let submission_policy = if open_proposal_submission {
        PreProposeSubmissionPolicy::Anyone { denylist: vec![] }
    } else {
        PreProposeSubmissionPolicy::Specific {
            dao_members: true,
            allowlist: vec![],
            denylist: vec![],
        }
    };

    dps::msg::InstantiateMsg {
        threshold: Threshold::AbsolutePercentage {
            percentage: PercentageThreshold::Majority {},
        },
        max_voting_period: cw_utils::Duration::Time(86400),
        min_voting_period: None,
        only_members_execute: false,
        allow_revoting: false,
        pre_propose_info: PreProposeInfo::ModuleMayPropose {
            info: ModuleInstantiateInfo {
                code_id: pre_propose_id,
                msg: to_json_binary(&InstantiateMsg {
                    deposit_info,
                    submission_policy,
                    extension: InstantiateExt {
                        approver: APPROVER.to_string(),
                    },
                })
                .unwrap(),
                admin: Some(Admin::CoreModule {}),
                funds: vec![],
                label: "baby's first pre-propose module, needs supervision".to_string(),
            },
        },
        close_proposal_on_execution_failure: false,
        veto: None,
    }
}

fn get_proposal_module_approver_instantiate(
    app: &mut App,
    _deposit_info: Option<UncheckedDepositInfo>,
    _open_proposal_submission: bool,
    pre_propose_approval_contract: String,
) -> dps::msg::InstantiateMsg {
    let pre_propose_id = app.store_code(dao_pre_propose_approver_contract());

    dps::msg::InstantiateMsg {
        threshold: Threshold::AbsolutePercentage {
            percentage: PercentageThreshold::Majority {},
        },
        max_voting_period: cw_utils::Duration::Time(86400),
        min_voting_period: None,
        only_members_execute: false,
        allow_revoting: false,
        pre_propose_info: PreProposeInfo::ModuleMayPropose {
            info: ModuleInstantiateInfo {
                code_id: pre_propose_id,
                msg: to_json_binary(&ApproverInstantiateMsg {
                    pre_propose_approval_contract,
                })
                .unwrap(),
                admin: Some(Admin::CoreModule {}),
                funds: vec![],
                label: "approver module".to_string(),
            },
        },
        close_proposal_on_execution_failure: false,
        veto: None,
    }
}

fn instantiate_cw20_base_default(app: &mut App) -> Addr {
    let cw20_id = app.store_code(cw20_base_contract());
    let cw20_instantiate = cw20_base::msg::InstantiateMsg {
        name: "cw20 token".to_string(),
        symbol: "cwtwenty".to_string(),
        decimals: 6,
        initial_balances: vec![Cw20Coin {
            address: "ekez".to_string(),
            amount: Uint128::new(10),
        }],
        mint: None,
        marketing: None,
    };
    app.instantiate_contract(
        cw20_id,
        Addr::unchecked("ekez"),
        &cw20_instantiate,
        &[],
        "cw20-base",
        None,
    )
    .unwrap()
}

struct DefaultTestSetup {
    core_addr: Addr,
    proposal_single: Addr,
    pre_propose: Addr,
    approver_core_addr: Addr,
    pre_propose_approver: Addr,
    proposal_single_approver: Addr,
}

fn setup_default_test(
    app: &mut App,
    deposit_info: Option<UncheckedDepositInfo>,
    open_proposal_submission: bool,
) -> DefaultTestSetup {
    let dps_id = app.store_code(dao_proposal_single_contract());

    // Instantiate SubDAO with pre-propose-approval-single
    let proposal_module_instantiate = get_proposal_module_approval_single_instantiate(
        app,
        deposit_info.clone(),
        open_proposal_submission,
    );
    let core_addr = instantiate_with_cw4_groups_governance(
        app,
        dps_id,
        to_json_binary(&proposal_module_instantiate).unwrap(),
        Some(vec![
            cw20::Cw20Coin {
                address: "ekez".to_string(),
                amount: Uint128::new(9),
            },
            cw20::Cw20Coin {
                address: "keze".to_string(),
                amount: Uint128::new(8),
            },
        ]),
    );
    let proposal_modules: Vec<ProposalModule> = app
        .wrap()
        .query_wasm_smart(
            core_addr.clone(),
            &dao_interface::msg::QueryMsg::ProposalModules {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();

    // Make sure things were set up correctly.
    assert_eq!(proposal_modules.len(), 1);
    let proposal_single = proposal_modules.into_iter().next().unwrap().address;
    let proposal_creation_policy = app
        .wrap()
        .query_wasm_smart(
            proposal_single.clone(),
            &dps::msg::QueryMsg::ProposalCreationPolicy {},
        )
        .unwrap();
    let pre_propose = match proposal_creation_policy {
        ProposalCreationPolicy::Module { addr } => addr,
        _ => panic!("expected a module for the proposal creation policy"),
    };
    assert_eq!(
        proposal_single,
        get_proposal_module(app, pre_propose.clone())
    );
    assert_eq!(core_addr, get_dao(app, pre_propose.clone()));

    // Instantiate SubDAO with pre-propose-approver
    let proposal_module_instantiate = get_proposal_module_approver_instantiate(
        app,
        deposit_info,
        open_proposal_submission,
        pre_propose.to_string(),
    );

    let approver_core_addr = instantiate_with_cw4_groups_governance(
        app,
        dps_id,
        to_json_binary(&proposal_module_instantiate).unwrap(),
        Some(vec![
            cw20::Cw20Coin {
                address: "ekez".to_string(),
                amount: Uint128::new(9),
            },
            cw20::Cw20Coin {
                address: "keze".to_string(),
                amount: Uint128::new(8),
            },
        ]),
    );
    let proposal_modules: Vec<ProposalModule> = app
        .wrap()
        .query_wasm_smart(
            approver_core_addr.clone(),
            &dao_interface::msg::QueryMsg::ProposalModules {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();

    // Make sure things were set up correctly.
    assert_eq!(proposal_modules.len(), 1);
    let proposal_single_approver = proposal_modules.into_iter().next().unwrap().address;
    let proposal_creation_policy = app
        .wrap()
        .query_wasm_smart(
            proposal_single_approver.clone(),
            &dps::msg::QueryMsg::ProposalCreationPolicy {},
        )
        .unwrap();
    let pre_propose_approver = match proposal_creation_policy {
        ProposalCreationPolicy::Module { addr } => addr,
        _ => panic!("expected a module for the proposal creation policy"),
    };
    assert_eq!(
        proposal_single_approver,
        get_proposal_module(app, pre_propose_approver.clone())
    );
    assert_eq!(
        approver_core_addr,
        get_dao(app, pre_propose_approver.clone())
    );
    assert_eq!(
        InfoResponse {
            info: ContractVersion {
                contract: "crates.io:dao-pre-propose-approver".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string()
            }
        },
        get_info(app, pre_propose_approver.clone())
    );

    DefaultTestSetup {
        core_addr,
        proposal_single,
        pre_propose,
        approver_core_addr,
        proposal_single_approver,
        pre_propose_approver,
    }
}

fn make_pre_proposal(app: &mut App, pre_propose: Addr, proposer: &str, funds: &[Coin]) -> u64 {
    app.execute_contract(
        Addr::unchecked(proposer),
        pre_propose.clone(),
        &ExecuteMsg::Propose {
            msg: ProposeMessage::Propose {
                title: "title".to_string(),
                description: "description".to_string(),
                msgs: vec![],
                vote: None,
            },
        },
        funds,
    )
    .unwrap();

    // Query for pending proposal and return latest id
    let mut pending: Vec<Proposal> = app
        .wrap()
        .query_wasm_smart(
            pre_propose,
            &QueryMsg::QueryExtension {
                msg: QueryExt::PendingProposals {
                    start_after: None,
                    limit: None,
                },
            },
        )
        .unwrap();

    // Return last item in list, id is first element of tuple
    pending.pop().unwrap().approval_id
}

fn mint_natives(app: &mut App, receiver: &str, coins: Vec<Coin>) {
    // Mint some ekez tokens for ekez so we can pay the deposit.
    app.sudo(cw_multi_test::SudoMsg::Bank(BankSudo::Mint {
        to_address: receiver.to_string(),
        amount: coins,
    }))
    .unwrap();
}

fn increase_allowance(app: &mut App, sender: &str, receiver: &Addr, cw20: Addr, amount: Uint128) {
    app.execute_contract(
        Addr::unchecked(sender),
        cw20,
        &cw20::Cw20ExecuteMsg::IncreaseAllowance {
            spender: receiver.to_string(),
            amount,
            expires: None,
        },
        &[],
    )
    .unwrap();
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

fn get_balance_native(app: &App, who: &str, denom: &str) -> Uint128 {
    let res = app.wrap().query_balance(who, denom).unwrap();
    res.amount
}

fn vote(app: &mut App, module: Addr, sender: &str, id: u64, position: Vote) -> Status {
    app.execute_contract(
        Addr::unchecked(sender),
        module.clone(),
        &dps::msg::ExecuteMsg::Vote {
            proposal_id: id,
            vote: position,
            rationale: None,
        },
        &[],
    )
    .unwrap();

    let proposal: ProposalResponse = app
        .wrap()
        .query_wasm_smart(module, &dps::msg::QueryMsg::Proposal { proposal_id: id })
        .unwrap();

    proposal.proposal.status
}

fn get_config(app: &App, module: Addr) -> Config {
    app.wrap()
        .query_wasm_smart(module, &QueryMsg::Config {})
        .unwrap()
}

fn get_dao(app: &App, module: Addr) -> Addr {
    app.wrap()
        .query_wasm_smart(module, &QueryMsg::Dao {})
        .unwrap()
}

fn get_info(app: &App, module: Addr) -> InfoResponse {
    app.wrap()
        .query_wasm_smart(module, &QueryMsg::Info {})
        .unwrap()
}

fn get_proposal_module(app: &App, module: Addr) -> Addr {
    app.wrap()
        .query_wasm_smart(module, &QueryMsg::ProposalModule {})
        .unwrap()
}

fn get_deposit_info(app: &App, module: Addr, id: u64) -> DepositInfoResponse {
    app.wrap()
        .query_wasm_smart(module, &QueryMsg::DepositInfo { proposal_id: id })
        .unwrap()
}

fn get_proposals(app: &App, module: Addr) -> ProposalListResponse {
    app.wrap()
        .query_wasm_smart(
            module,
            &dps::msg::QueryMsg::ListProposals {
                start_after: None,
                limit: None,
            },
        )
        .unwrap()
}

fn get_latest_proposal_id(app: &App, module: Addr) -> u64 {
    // Check prop was created in the main DAO
    let props: ProposalListResponse = app
        .wrap()
        .query_wasm_smart(
            module,
            &dps::msg::QueryMsg::ListProposals {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    props.proposals[props.proposals.len() - 1].id
}

fn query_can_propose(app: &App, module: Addr, address: impl Into<String>) -> bool {
    app.wrap()
        .query_wasm_smart(
            module,
            &QueryMsg::CanPropose {
                address: address.into(),
            },
        )
        .unwrap()
}

fn update_config(
    app: &mut App,
    module: Addr,
    sender: &str,
    deposit_info: Option<UncheckedDepositInfo>,
    submission_policy: PreProposeSubmissionPolicy,
) -> Config {
    app.execute_contract(
        Addr::unchecked(sender),
        module.clone(),
        &ExecuteMsg::UpdateConfig {
            deposit_info,
            submission_policy: Some(submission_policy),
        },
        &[],
    )
    .unwrap();

    get_config(app, module)
}

fn update_config_should_fail(
    app: &mut App,
    module: Addr,
    sender: &str,
    deposit_info: Option<UncheckedDepositInfo>,
    submission_policy: PreProposeSubmissionPolicy,
) -> PreProposeError {
    app.execute_contract(
        Addr::unchecked(sender),
        module,
        &ExecuteMsg::UpdateConfig {
            deposit_info,
            submission_policy: Some(submission_policy),
        },
        &[],
    )
    .unwrap_err()
    .downcast()
    .unwrap()
}

fn withdraw(app: &mut App, module: Addr, sender: &str, denom: Option<UncheckedDenom>) {
    app.execute_contract(
        Addr::unchecked(sender),
        module,
        &ExecuteMsg::Withdraw { denom },
        &[],
    )
    .unwrap();
}

fn withdraw_should_fail(
    app: &mut App,
    module: Addr,
    sender: &str,
    denom: Option<UncheckedDenom>,
) -> PreProposeError {
    app.execute_contract(
        Addr::unchecked(sender),
        module,
        &ExecuteMsg::Withdraw { denom },
        &[],
    )
    .unwrap_err()
    .downcast()
    .unwrap()
}

fn close_proposal(app: &mut App, module: Addr, sender: &str, proposal_id: u64) {
    app.execute_contract(
        Addr::unchecked(sender),
        module,
        &dps::msg::ExecuteMsg::Close { proposal_id },
        &[],
    )
    .unwrap();
}

fn execute_proposal(app: &mut App, module: Addr, sender: &str, proposal_id: u64) {
    app.execute_contract(
        Addr::unchecked(sender),
        module,
        &dps::msg::ExecuteMsg::Execute { proposal_id },
        &[],
    )
    .unwrap();
}

fn approve_proposal(app: &mut App, module: Addr, sender: &str, proposal_id: u64) {
    // Approver votes on prop
    vote(app, module.clone(), sender, proposal_id, Vote::Yes);
    // Approver executes prop
    execute_proposal(app, module, sender, proposal_id);
}

enum ApprovalStatus {
    Approved,
    Rejected,
}

enum EndStatus {
    Passed,
    Failed,
}

enum RefundReceiver {
    Proposer,
    Dao,
}

fn test_native_permutation(
    end_status: EndStatus,
    refund_policy: DepositRefundPolicy,
    receiver: RefundReceiver,
    approval_status: ApprovalStatus,
) {
    let mut app = App::default();

    // Need to instantiate this so contract addresses match with cw20 test cases
    let _ = instantiate_cw20_base_default(&mut app);

    let DefaultTestSetup {
        core_addr,
        proposal_single,
        pre_propose,
        approver_core_addr: _,
        proposal_single_approver,
        pre_propose_approver: _,
    } = setup_default_test(
        &mut app,
        Some(UncheckedDepositInfo {
            denom: DepositToken::Token {
                denom: UncheckedDenom::Native("ujuno".to_string()),
            },
            amount: Uint128::new(10),
            refund_policy,
        }),
        false,
    );

    mint_natives(&mut app, "ekez", coins(10, "ujuno"));
    let _pre_propose_id = make_pre_proposal(&mut app, pre_propose, "ekez", &coins(10, "ujuno"));

    // Check no props created on main DAO yet
    let props = get_proposals(&app, proposal_single.clone());
    assert_eq!(props.proposals.len(), 0);

    // Make sure it went away.
    let balance = get_balance_native(&app, "ekez", "ujuno");
    assert_eq!(balance, Uint128::zero());

    // Approver approves or rejects proposal
    match approval_status {
        ApprovalStatus::Approved => {
            // Get approver proposal id
            let id = get_latest_proposal_id(&app, proposal_single_approver.clone());

            // Approver votes on prop
            vote(
                &mut app,
                proposal_single_approver.clone(),
                "ekez",
                id,
                Vote::Yes,
            );
            // Approver executes prop
            execute_proposal(&mut app, proposal_single_approver, "ekez", id);

            // Check prop was created in the main DAO
            let id = get_latest_proposal_id(&app, proposal_single.clone());
            let props = get_proposals(&app, proposal_single.clone());
            assert_eq!(props.proposals.len(), 1);

            // Voting happens on newly created proposal
            #[allow(clippy::type_complexity)]
            let (position, expected_status, trigger_refund): (
                _,
                _,
                fn(&mut App, Addr, &str, u64) -> (),
            ) = match end_status {
                EndStatus::Passed => (Vote::Yes, Status::Passed, execute_proposal),
                EndStatus::Failed => (Vote::No, Status::Rejected, close_proposal),
            };
            let new_status = vote(&mut app, proposal_single.clone(), "ekez", id, position);
            assert_eq!(new_status, expected_status);

            // Close or execute the proposal to trigger a refund.
            trigger_refund(&mut app, proposal_single, "ekez", id);
        }
        ApprovalStatus::Rejected => {
            // Approver votes on prop
            // No proposal is created so there is no voting
            vote(
                &mut app,
                proposal_single_approver.clone(),
                "ekez",
                1,
                Vote::No,
            );
            // Approver executes prop
            close_proposal(&mut app, proposal_single_approver, "ekez", 1);

            // No prop created
            let props = get_proposals(&app, proposal_single);
            assert_eq!(props.proposals.len(), 0);
        }
    };

    let (dao_expected, proposer_expected) = match receiver {
        RefundReceiver::Proposer => (0, 10),
        RefundReceiver::Dao => (10, 0),
    };

    let proposer_balance = get_balance_native(&app, "ekez", "ujuno");
    let dao_balance = get_balance_native(&app, core_addr.as_str(), "ujuno");
    assert_eq!(proposer_expected, proposer_balance.u128());
    assert_eq!(dao_expected, dao_balance.u128())
}

fn test_cw20_permutation(
    end_status: EndStatus,
    refund_policy: DepositRefundPolicy,
    receiver: RefundReceiver,
    approval_status: ApprovalStatus,
) {
    let mut app = App::default();

    let cw20_address = instantiate_cw20_base_default(&mut app);

    let DefaultTestSetup {
        core_addr,
        proposal_single,
        pre_propose,
        approver_core_addr: _,
        proposal_single_approver,
        pre_propose_approver: _,
    } = setup_default_test(
        &mut app,
        Some(UncheckedDepositInfo {
            denom: DepositToken::Token {
                denom: UncheckedDenom::Cw20(cw20_address.to_string()),
            },
            amount: Uint128::new(10),
            refund_policy,
        }),
        false,
    );

    increase_allowance(
        &mut app,
        "ekez",
        &pre_propose,
        cw20_address.clone(),
        Uint128::new(10),
    );
    let _pre_propose_id = make_pre_proposal(&mut app, pre_propose.clone(), "ekez", &[]);

    // Check no props created on main DAO yet
    let props = get_proposals(&app, proposal_single.clone());
    assert_eq!(props.proposals.len(), 0);

    // Make sure it went await.
    let balance = get_balance_cw20(&app, cw20_address.clone(), "ekez");
    assert_eq!(balance, Uint128::zero());

    // Approver approves or rejects proposal
    match approval_status {
        ApprovalStatus::Approved => {
            // Get approver proposal id
            let id = get_latest_proposal_id(&app, proposal_single_approver.clone());

            // Approver votes on prop
            vote(
                &mut app,
                proposal_single_approver.clone(),
                "ekez",
                id,
                Vote::Yes,
            );
            // Approver executes prop
            execute_proposal(&mut app, proposal_single_approver, "ekez", id);

            // Check prop was created in the main DAO
            let id = get_latest_proposal_id(&app, proposal_single.clone());
            let props = get_proposals(&app, proposal_single.clone());
            assert_eq!(props.proposals.len(), 1);

            // Voting happens on newly created proposal
            #[allow(clippy::type_complexity)]
            let (position, expected_status, trigger_refund): (
                _,
                _,
                fn(&mut App, Addr, &str, u64) -> (),
            ) = match end_status {
                EndStatus::Passed => (Vote::Yes, Status::Passed, execute_proposal),
                EndStatus::Failed => (Vote::No, Status::Rejected, close_proposal),
            };
            let new_status = vote(&mut app, proposal_single.clone(), "ekez", id, position);
            assert_eq!(new_status, expected_status);

            // Close or execute the proposal to trigger a refund.
            trigger_refund(&mut app, proposal_single, "ekez", id);
        }
        ApprovalStatus::Rejected => {
            // Approver votes on prop
            // No proposal is created so there is no voting
            vote(
                &mut app,
                proposal_single_approver.clone(),
                "ekez",
                1,
                Vote::No,
            );
            // Approver executes prop
            close_proposal(&mut app, proposal_single_approver, "ekez", 1);

            // No prop created
            let props = get_proposals(&app, proposal_single);
            assert_eq!(props.proposals.len(), 0);
        }
    };

    let (dao_expected, proposer_expected) = match receiver {
        RefundReceiver::Proposer => (0, 10),
        RefundReceiver::Dao => (10, 0),
    };

    let proposer_balance = get_balance_cw20(&app, &cw20_address, "ekez");
    let dao_balance = get_balance_cw20(&app, &cw20_address, core_addr);
    assert_eq!(proposer_expected, proposer_balance.u128());
    assert_eq!(dao_expected, dao_balance.u128())
}

#[test]
fn test_native_failed_always_refund() {
    test_native_permutation(
        EndStatus::Failed,
        DepositRefundPolicy::Always,
        RefundReceiver::Proposer,
        ApprovalStatus::Approved,
    )
}

#[test]
fn test_native_rejected_always_refund() {
    test_native_permutation(
        EndStatus::Failed,
        DepositRefundPolicy::Always,
        RefundReceiver::Proposer,
        ApprovalStatus::Rejected,
    )
}

#[test]
fn test_cw20_failed_always_refund() {
    test_cw20_permutation(
        EndStatus::Failed,
        DepositRefundPolicy::Always,
        RefundReceiver::Proposer,
        ApprovalStatus::Approved,
    )
}

#[test]
fn test_cw20_rejected_always_refund() {
    test_cw20_permutation(
        EndStatus::Failed,
        DepositRefundPolicy::Always,
        RefundReceiver::Proposer,
        ApprovalStatus::Rejected,
    )
}

#[test]
fn test_native_passed_always_refund() {
    test_native_permutation(
        EndStatus::Passed,
        DepositRefundPolicy::Always,
        RefundReceiver::Proposer,
        ApprovalStatus::Approved,
    )
}

#[test]
fn test_cw20_passed_always_refund() {
    test_cw20_permutation(
        EndStatus::Passed,
        DepositRefundPolicy::Always,
        RefundReceiver::Proposer,
        ApprovalStatus::Approved,
    )
}

#[test]
fn test_native_passed_never_refund() {
    test_native_permutation(
        EndStatus::Passed,
        DepositRefundPolicy::Never,
        RefundReceiver::Dao,
        ApprovalStatus::Approved,
    )
}

#[test]
fn test_cw20_passed_never_refund() {
    test_cw20_permutation(
        EndStatus::Passed,
        DepositRefundPolicy::Never,
        RefundReceiver::Dao,
        ApprovalStatus::Approved,
    )
}

#[test]
fn test_native_failed_never_refund() {
    test_native_permutation(
        EndStatus::Failed,
        DepositRefundPolicy::Never,
        RefundReceiver::Dao,
        ApprovalStatus::Approved,
    )
}

#[test]
fn test_native_rejected_never_refund() {
    test_native_permutation(
        EndStatus::Failed,
        DepositRefundPolicy::Never,
        RefundReceiver::Dao,
        ApprovalStatus::Rejected,
    )
}

#[test]
fn test_cw20_failed_never_refund() {
    test_cw20_permutation(
        EndStatus::Failed,
        DepositRefundPolicy::Never,
        RefundReceiver::Dao,
        ApprovalStatus::Approved,
    )
}

#[test]
fn test_cw20_rejected_never_refund() {
    test_cw20_permutation(
        EndStatus::Failed,
        DepositRefundPolicy::Never,
        RefundReceiver::Dao,
        ApprovalStatus::Rejected,
    )
}

#[test]
fn test_native_passed_passed_refund() {
    test_native_permutation(
        EndStatus::Passed,
        DepositRefundPolicy::OnlyPassed,
        RefundReceiver::Proposer,
        ApprovalStatus::Approved,
    )
}

#[test]
fn test_cw20_passed_passed_refund() {
    test_cw20_permutation(
        EndStatus::Passed,
        DepositRefundPolicy::OnlyPassed,
        RefundReceiver::Proposer,
        ApprovalStatus::Approved,
    )
}

#[test]
fn test_native_failed_passed_refund() {
    test_native_permutation(
        EndStatus::Failed,
        DepositRefundPolicy::OnlyPassed,
        RefundReceiver::Dao,
        ApprovalStatus::Approved,
    )
}

#[test]
fn test_native_rejected_passed_refund() {
    test_native_permutation(
        EndStatus::Failed,
        DepositRefundPolicy::OnlyPassed,
        RefundReceiver::Dao,
        ApprovalStatus::Rejected,
    )
}

#[test]
fn test_cw20_failed_passed_refund() {
    test_cw20_permutation(
        EndStatus::Failed,
        DepositRefundPolicy::OnlyPassed,
        RefundReceiver::Dao,
        ApprovalStatus::Approved,
    )
}

#[test]
fn test_cw20_rejected_passed_refund() {
    test_cw20_permutation(
        EndStatus::Failed,
        DepositRefundPolicy::OnlyPassed,
        RefundReceiver::Dao,
        ApprovalStatus::Rejected,
    )
}

// See: <https://github.com/DA0-DA0/dao-contracts/pull/465#discussion_r960092321>
#[test]
fn test_multiple_open_proposals() {
    let mut app = App::default();

    // Need to instantiate this so contract addresses match with cw20 test cases
    let _ = instantiate_cw20_base_default(&mut app);

    let DefaultTestSetup {
        core_addr: _,
        proposal_single,
        pre_propose,
        approver_core_addr: _,
        proposal_single_approver,
        pre_propose_approver: _,
    } = setup_default_test(
        &mut app,
        Some(UncheckedDepositInfo {
            denom: DepositToken::Token {
                denom: UncheckedDenom::Native("ujuno".to_string()),
            },
            amount: Uint128::new(10),
            refund_policy: DepositRefundPolicy::Always,
        }),
        false,
    );

    mint_natives(&mut app, "ekez", coins(20, "ujuno"));
    let _first_pre_propose_id =
        make_pre_proposal(&mut app, pre_propose.clone(), "ekez", &coins(10, "ujuno"));
    let balance = get_balance_native(&app, "ekez", "ujuno");
    assert_eq!(10, balance.u128());

    // Approver DAO approves prop, balance remains the same
    let approver_prop_id = get_latest_proposal_id(&app, proposal_single_approver.clone());
    approve_proposal(
        &mut app,
        proposal_single_approver.clone(),
        "ekez",
        approver_prop_id,
    );
    let first_id = get_latest_proposal_id(&app, proposal_single.clone());
    let balance = get_balance_native(&app, "ekez", "ujuno");
    assert_eq!(10, balance.u128());

    let _second_pre_propose_id =
        make_pre_proposal(&mut app, pre_propose, "ekez", &coins(10, "ujuno"));
    let balance = get_balance_native(&app, "ekez", "ujuno");
    assert_eq!(0, balance.u128());

    // Approver DAO votes to approves, balance remains the same
    let approver_prop_id = get_latest_proposal_id(&app, proposal_single_approver.clone());
    approve_proposal(&mut app, proposal_single_approver, "ekez", approver_prop_id);
    let second_id = get_latest_proposal_id(&app, proposal_single.clone());
    let balance = get_balance_native(&app, "ekez", "ujuno");
    assert_eq!(0, balance.u128());

    // Finish up the first proposal.
    let new_status = vote(
        &mut app,
        proposal_single.clone(),
        "ekez",
        first_id,
        Vote::Yes,
    );
    assert_eq!(Status::Passed, new_status);

    // Still zero.
    let balance = get_balance_native(&app, "ekez", "ujuno");
    assert_eq!(0, balance.u128());

    execute_proposal(&mut app, proposal_single.clone(), "ekez", first_id);

    // First proposal refunded.
    let balance = get_balance_native(&app, "ekez", "ujuno");
    assert_eq!(10, balance.u128());

    // Finish up the second proposal.
    let new_status = vote(
        &mut app,
        proposal_single.clone(),
        "ekez",
        second_id,
        Vote::No,
    );
    assert_eq!(Status::Rejected, new_status);

    // Still zero.
    let balance = get_balance_native(&app, "ekez", "ujuno");
    assert_eq!(10, balance.u128());

    close_proposal(&mut app, proposal_single, "ekez", second_id);

    // All deposits have been refunded.
    let balance = get_balance_native(&app, "ekez", "ujuno");
    assert_eq!(20, balance.u128());
}

#[test]
fn test_set_version() {
    let mut app = App::default();

    // Need to instantiate this so contract addresses match with cw20 test cases
    let _ = instantiate_cw20_base_default(&mut app);

    let DefaultTestSetup {
        core_addr: _,
        proposal_single: _,
        pre_propose: _,
        approver_core_addr: _,
        proposal_single_approver: _,
        pre_propose_approver,
    } = setup_default_test(
        &mut app,
        Some(UncheckedDepositInfo {
            denom: DepositToken::Token {
                denom: UncheckedDenom::Native("ujuno".to_string()),
            },
            amount: Uint128::new(10),
            refund_policy: DepositRefundPolicy::Always,
        }),
        false,
    );

    let info: ContractVersion = from_json(
        app.wrap()
            .query_wasm_raw(pre_propose_approver, "contract_info".as_bytes())
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        ContractVersion {
            contract: CONTRACT_NAME.to_string(),
            version: CONTRACT_VERSION.to_string()
        },
        info
    )
}

#[test]
fn test_permissions() {
    let mut app = App::default();

    // Need to instantiate this so contract addresses match with cw20 test cases
    let _ = instantiate_cw20_base_default(&mut app);

    let DefaultTestSetup {
        core_addr,
        proposal_single: _,
        pre_propose,
        approver_core_addr: _,
        proposal_single_approver: _,
        pre_propose_approver: _,
    } = setup_default_test(
        &mut app,
        Some(UncheckedDepositInfo {
            denom: DepositToken::Token {
                denom: UncheckedDenom::Native("ujuno".to_string()),
            },
            amount: Uint128::new(10),
            refund_policy: DepositRefundPolicy::Always,
        }),
        false, // no open proposal submission.
    );

    let err: PreProposeError = app
        .execute_contract(
            core_addr,
            pre_propose.clone(),
            &ExecuteMsg::ProposalCompletedHook {
                proposal_id: 1,
                new_status: Status::Closed,
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, PreProposeError::NotModule {});

    // Non-members may not propose when open_propose_submission is
    // disabled.
    let err: PreProposeError = app
        .execute_contract(
            Addr::unchecked("nonmember"),
            pre_propose,
            &ExecuteMsg::Propose {
                msg: ProposeMessage::Propose {
                    title: "I would like to join the DAO".to_string(),
                    description: "though, I am currently not a member.".to_string(),
                    msgs: vec![],
                    vote: None,
                },
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(
        err,
        PreProposeError::SubmissionPolicy(PreProposeSubmissionPolicyError::Unauthorized {})
    );
}

#[test]
fn test_approval_and_rejection_permissions() {
    let mut app = App::default();

    // Need to instantiate this so contract addresses match with cw20 test cases
    let _ = instantiate_cw20_base_default(&mut app);

    let DefaultTestSetup {
        core_addr: _,
        proposal_single: _,
        pre_propose,
        approver_core_addr: _,
        proposal_single_approver: _,
        pre_propose_approver: _,
    } = setup_default_test(
        &mut app,
        Some(UncheckedDepositInfo {
            denom: DepositToken::Token {
                denom: UncheckedDenom::Native("ujuno".to_string()),
            },
            amount: Uint128::new(10),
            refund_policy: DepositRefundPolicy::Always,
        }),
        true, // yes, open proposal submission.
    );

    // Non-member proposes.
    mint_natives(&mut app, "nonmember", coins(10, "ujuno"));
    let pre_propose_id = make_pre_proposal(
        &mut app,
        pre_propose.clone(),
        "nonmember",
        &coins(10, "ujuno"),
    );

    // Only approver can propose
    let err: PreProposeError = app
        .execute_contract(
            Addr::unchecked("nonmember"),
            pre_propose.clone(),
            &ExecuteMsg::Extension {
                msg: ExecuteExt::Approve { id: pre_propose_id },
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, PreProposeError::Unauthorized {});

    // Only approver can propose
    let err: PreProposeError = app
        .execute_contract(
            Addr::unchecked("nonmember"),
            pre_propose,
            &ExecuteMsg::Extension {
                msg: ExecuteExt::Reject { id: pre_propose_id },
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, PreProposeError::Unauthorized {});
}

#[test]
fn test_propose_open_proposal_submission() {
    let mut app = App::default();

    // Need to instantiate this so contract addresses match with cw20 test cases
    let _ = instantiate_cw20_base_default(&mut app);

    let DefaultTestSetup {
        core_addr: _,
        proposal_single,
        pre_propose,
        approver_core_addr: _,
        proposal_single_approver,
        pre_propose_approver,
    } = setup_default_test(
        &mut app,
        Some(UncheckedDepositInfo {
            denom: DepositToken::Token {
                denom: UncheckedDenom::Native("ujuno".to_string()),
            },
            amount: Uint128::new(10),
            refund_policy: DepositRefundPolicy::Always,
        }),
        true, // yes, open proposal submission.
    );

    // Non-member proposes.
    mint_natives(&mut app, "nonmember", coins(10, "ujuno"));
    let pre_propose_id = make_pre_proposal(&mut app, pre_propose, "nonmember", &coins(10, "ujuno"));

    let approver_prop_id = get_latest_proposal_id(&app, proposal_single_approver.clone());
    let pre_propose_id_from_proposal: u64 = app
        .wrap()
        .query_wasm_smart(
            pre_propose_approver.clone(),
            &ApproverQueryMsg::QueryExtension {
                msg: ApproverQueryExt::PreProposeApprovalIdForApproverProposalId {
                    id: approver_prop_id,
                },
            },
        )
        .unwrap();
    assert_eq!(pre_propose_id_from_proposal, pre_propose_id);

    let proposal_id_from_pre_propose: u64 = app
        .wrap()
        .query_wasm_smart(
            pre_propose_approver.clone(),
            &ApproverQueryMsg::QueryExtension {
                msg: ApproverQueryExt::ApproverProposalIdForPreProposeApprovalId {
                    id: pre_propose_id,
                },
            },
        )
        .unwrap();
    assert_eq!(proposal_id_from_pre_propose, approver_prop_id);

    // Approver DAO votes to approves
    approve_proposal(&mut app, proposal_single_approver, "ekez", approver_prop_id);
    let id = get_latest_proposal_id(&app, proposal_single.clone());

    // Member votes.
    let new_status = vote(&mut app, proposal_single, "ekez", id, Vote::Yes);
    assert_eq!(Status::Passed, new_status)
}

#[test]
fn test_update_config() {
    let mut app = App::default();

    // Need to instantiate this so contract addresses match with cw20 test cases
    let _ = instantiate_cw20_base_default(&mut app);

    let DefaultTestSetup {
        core_addr,
        proposal_single,
        pre_propose,
        approver_core_addr: _,
        proposal_single_approver,
        pre_propose_approver: _,
    } = setup_default_test(&mut app, None, false);

    let config = get_config(&app, pre_propose.clone());
    assert_eq!(
        config,
        Config {
            deposit_info: None,
            submission_policy: PreProposeSubmissionPolicy::Specific {
                dao_members: true,
                allowlist: vec![],
                denylist: vec![]
            }
        }
    );

    let _pre_propose_id = make_pre_proposal(&mut app, pre_propose.clone(), "ekez", &[]);

    // Approver DAO votes to approves
    let approver_prop_id = get_latest_proposal_id(&app, proposal_single_approver.clone());
    approve_proposal(
        &mut app,
        proposal_single_approver.clone(),
        "ekez",
        approver_prop_id,
    );
    let id = get_latest_proposal_id(&app, proposal_single.clone());

    update_config(
        &mut app,
        pre_propose.clone(),
        core_addr.as_str(),
        Some(UncheckedDepositInfo {
            denom: DepositToken::Token {
                denom: UncheckedDenom::Native("ujuno".to_string()),
            },
            amount: Uint128::new(10),
            refund_policy: DepositRefundPolicy::Never,
        }),
        PreProposeSubmissionPolicy::Anyone { denylist: vec![] },
    );

    let config = get_config(&app, pre_propose.clone());
    assert_eq!(
        config,
        Config {
            deposit_info: Some(CheckedDepositInfo {
                denom: cw_denom::CheckedDenom::Native("ujuno".to_string()),
                amount: Uint128::new(10),
                refund_policy: DepositRefundPolicy::Never
            }),
            submission_policy: PreProposeSubmissionPolicy::Anyone { denylist: vec![] },
        }
    );

    // Old proposal should still have same deposit info.
    let info = get_deposit_info(&app, pre_propose.clone(), id);
    assert_eq!(
        info,
        DepositInfoResponse {
            deposit_info: None,
            proposer: Addr::unchecked("ekez"),
        }
    );

    // New proposals should have the new deposit info.
    mint_natives(&mut app, "ekez", coins(10, "ujuno"));
    let _new_pre_propose_id =
        make_pre_proposal(&mut app, pre_propose.clone(), "ekez", &coins(10, "ujuno"));

    // Approver DAO votes to approve prop
    let approver_prop_id = get_latest_proposal_id(&app, proposal_single_approver.clone());
    approve_proposal(
        &mut app,
        proposal_single_approver.clone(),
        "ekez",
        approver_prop_id,
    );
    let new_id = get_latest_proposal_id(&app, proposal_single_approver);

    let info = get_deposit_info(&app, pre_propose.clone(), new_id);
    assert_eq!(
        info,
        DepositInfoResponse {
            deposit_info: Some(CheckedDepositInfo {
                denom: cw_denom::CheckedDenom::Native("ujuno".to_string()),
                amount: Uint128::new(10),
                refund_policy: DepositRefundPolicy::Never
            }),
            proposer: Addr::unchecked("ekez"),
        }
    );

    // Both proposals should be allowed to complete.
    vote(&mut app, proposal_single.clone(), "ekez", id, Vote::Yes);
    vote(&mut app, proposal_single.clone(), "ekez", new_id, Vote::Yes);
    execute_proposal(&mut app, proposal_single.clone(), "ekez", id);
    execute_proposal(&mut app, proposal_single.clone(), "ekez", new_id);
    // Deposit should not have been refunded (never policy in use).
    let balance = get_balance_native(&app, "ekez", "ujuno");
    assert_eq!(balance, Uint128::new(0));

    // Only the core module can update the config.
    let err = update_config_should_fail(
        &mut app,
        pre_propose.clone(),
        proposal_single.as_str(),
        None,
        PreProposeSubmissionPolicy::Anyone { denylist: vec![] },
    );
    assert_eq!(err, PreProposeError::NotDao {});

    // Errors when no one is authorized to create proposals.
    let err = update_config_should_fail(
        &mut app,
        pre_propose.clone(),
        core_addr.as_str(),
        None,
        PreProposeSubmissionPolicy::Specific {
            dao_members: false,
            allowlist: vec![],
            denylist: vec![],
        },
    );
    assert_eq!(
        err,
        PreProposeError::SubmissionPolicy(PreProposeSubmissionPolicyError::NoOneAllowed {})
    );

    // Errors when allowlist and denylist overlap.
    let err = update_config_should_fail(
        &mut app,
        pre_propose,
        core_addr.as_str(),
        None,
        PreProposeSubmissionPolicy::Specific {
            dao_members: false,
            allowlist: vec![Addr::unchecked("ekez")],
            denylist: vec![Addr::unchecked("ekez")],
        },
    );
    assert_eq!(
        err,
        PreProposeError::SubmissionPolicy(
            PreProposeSubmissionPolicyError::DenylistAllowlistOverlap {}
        )
    );
}

#[test]
fn test_approver_unsupported_update_config() {
    let mut app = App::default();

    // Need to instantiate this so contract addresses match with cw20 test cases
    let _ = instantiate_cw20_base_default(&mut app);

    let DefaultTestSetup {
        core_addr,
        pre_propose_approver,
        ..
    } = setup_default_test(&mut app, None, true);

    // Should fail because config is not supported for the approver pre-propose
    // contract.
    let err = update_config_should_fail(
        &mut app,
        pre_propose_approver,
        core_addr.as_str(),
        None,
        PreProposeSubmissionPolicy::Specific {
            dao_members: false,
            allowlist: vec![Addr::unchecked("ekez")],
            denylist: vec![],
        },
    );
    assert_eq!(err, PreProposeError::Unsupported {});
}

#[test]
fn test_approver_unsupported_update_submission_policy() {
    let mut app = App::default();

    // Need to instantiate this so contract addresses match with cw20 test cases
    let _ = instantiate_cw20_base_default(&mut app);

    let DefaultTestSetup {
        core_addr,
        pre_propose_approver,
        ..
    } = setup_default_test(&mut app, None, true);

    // Should fail because submission policy is not supported for the approver
    // pre-propose contract.
    let err: PreProposeError = app
        .execute_contract(
            core_addr,
            pre_propose_approver,
            &ExecuteMsg::UpdateSubmissionPolicy {
                denylist_add: Some(vec!["ekez".to_string()]),
                denylist_remove: None,
                set_dao_members: None,
                allowlist_add: None,
                allowlist_remove: None,
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, PreProposeError::Unsupported {});
}

#[test]
fn test_approver_can_propose() {
    let mut app = App::default();

    // Need to instantiate this so contract addresses match with cw20 test cases
    let _ = instantiate_cw20_base_default(&mut app);

    let DefaultTestSetup {
        pre_propose,
        pre_propose_approver,
        ..
    } = setup_default_test(&mut app, None, true);

    // Only the pre-propose-approval-single contract can propose.
    assert!(query_can_propose(
        &app,
        pre_propose_approver.clone(),
        pre_propose
    ));
    assert!(!query_can_propose(
        &app,
        pre_propose_approver,
        "someone_else"
    ));
}

#[test]
fn test_withdraw() {
    let mut app = App::default();

    // Need to instantiate this so contract addresses match with cw20 test cases
    let _ = instantiate_cw20_base_default(&mut app);

    let DefaultTestSetup {
        core_addr,
        proposal_single,
        pre_propose,
        approver_core_addr: _,
        proposal_single_approver,
        pre_propose_approver: _,
    } = setup_default_test(&mut app, None, false);

    let err = withdraw_should_fail(
        &mut app,
        pre_propose.clone(),
        proposal_single.as_str(),
        Some(UncheckedDenom::Native("ujuno".to_string())),
    );
    assert_eq!(err, PreProposeError::NotDao {});

    let err = withdraw_should_fail(
        &mut app,
        pre_propose.clone(),
        core_addr.as_str(),
        Some(UncheckedDenom::Native("ujuno".to_string())),
    );
    assert_eq!(err, PreProposeError::NothingToWithdraw {});

    let err = withdraw_should_fail(&mut app, pre_propose.clone(), core_addr.as_str(), None);
    assert_eq!(err, PreProposeError::NoWithdrawalDenom {});

    // Turn on native deposits.
    update_config(
        &mut app,
        pre_propose.clone(),
        core_addr.as_str(),
        Some(UncheckedDepositInfo {
            denom: DepositToken::Token {
                denom: UncheckedDenom::Native("ujuno".to_string()),
            },
            amount: Uint128::new(10),
            refund_policy: DepositRefundPolicy::Always,
        }),
        PreProposeSubmissionPolicy::Specific {
            dao_members: true,
            allowlist: vec![],
            denylist: vec![],
        },
    );

    // Withdraw with no specified denom - should fall back to the one
    // in the config.
    mint_natives(&mut app, pre_propose.as_str(), coins(10, "ujuno"));
    withdraw(&mut app, pre_propose.clone(), core_addr.as_str(), None);
    let balance = get_balance_native(&app, core_addr.as_str(), "ujuno");
    assert_eq!(balance, Uint128::new(10));

    // Withdraw again, this time specifying a native denomination.
    mint_natives(&mut app, pre_propose.as_str(), coins(10, "ujuno"));
    withdraw(
        &mut app,
        pre_propose.clone(),
        core_addr.as_str(),
        Some(UncheckedDenom::Native("ujuno".to_string())),
    );
    let balance = get_balance_native(&app, core_addr.as_str(), "ujuno");
    assert_eq!(balance, Uint128::new(20));

    // Make a proposal with the native tokens to put some in the system.
    mint_natives(&mut app, "ekez", coins(10, "ujuno"));
    let _native_pre_propose_id =
        make_pre_proposal(&mut app, pre_propose.clone(), "ekez", &coins(10, "ujuno"));

    // Approver DAO votes to approve
    let approver_prop_id = get_latest_proposal_id(&app, proposal_single_approver.clone());
    approve_proposal(
        &mut app,
        proposal_single_approver.clone(),
        "ekez",
        approver_prop_id,
    );
    let native_id = get_latest_proposal_id(&app, proposal_single_approver.clone());

    // Update the config to use a cw20 token.
    let cw20_address = instantiate_cw20_base_default(&mut app);
    update_config(
        &mut app,
        pre_propose.clone(),
        core_addr.as_str(),
        Some(UncheckedDepositInfo {
            denom: DepositToken::Token {
                denom: UncheckedDenom::Cw20(cw20_address.to_string()),
            },
            amount: Uint128::new(10),
            refund_policy: DepositRefundPolicy::Always,
        }),
        PreProposeSubmissionPolicy::Specific {
            dao_members: true,
            allowlist: vec![],
            denylist: vec![],
        },
    );

    increase_allowance(
        &mut app,
        "ekez",
        &pre_propose,
        cw20_address.clone(),
        Uint128::new(10),
    );
    let _cw20_pre_propose_id = make_pre_proposal(&mut app, pre_propose.clone(), "ekez", &[]);

    // Approver DAO votes to approve
    let approver_prop_id = get_latest_proposal_id(&app, proposal_single_approver.clone());
    approve_proposal(&mut app, proposal_single_approver, "ekez", approver_prop_id);
    let cw20_id = get_latest_proposal_id(&app, proposal_single.clone());

    // There is now a pending proposal and cw20 tokens in the
    // pre-propose module that should be returned on that proposal's
    // completion. To make things interesting, we withdraw those
    // tokens which should cause the status change hook on the
    // proposal's execution to fail as we don't have sufficent balance
    // to return the deposit.
    withdraw(&mut app, pre_propose.clone(), core_addr.as_str(), None);
    let balance = get_balance_cw20(&app, &cw20_address, core_addr.as_str());
    assert_eq!(balance, Uint128::new(10));

    // Proposal should still be executable! We just get removed from
    // the proposal module's hook receiver list.
    vote(
        &mut app,
        proposal_single.clone(),
        "ekez",
        cw20_id,
        Vote::Yes,
    );
    execute_proposal(&mut app, proposal_single.clone(), "ekez", cw20_id);

    // Make sure the proposal module has fallen back to anyone can
    // propose becuase of our malfunction.
    let proposal_creation_policy: ProposalCreationPolicy = app
        .wrap()
        .query_wasm_smart(
            proposal_single.clone(),
            &dps::msg::QueryMsg::ProposalCreationPolicy {},
        )
        .unwrap();

    assert_eq!(proposal_creation_policy, ProposalCreationPolicy::Anyone {});

    // Close out the native proposal and it's deposit as well.
    vote(
        &mut app,
        proposal_single.clone(),
        "ekez",
        native_id,
        Vote::No,
    );
    close_proposal(&mut app, proposal_single.clone(), "ekez", native_id);
    withdraw(
        &mut app,
        pre_propose.clone(),
        core_addr.as_str(),
        Some(UncheckedDenom::Native("ujuno".to_string())),
    );
    let balance = get_balance_native(&app, core_addr.as_str(), "ujuno");
    assert_eq!(balance, Uint128::new(30));
}

#[test]
fn test_reset_approver() {
    let mut app = App::default();

    // Need to instantiate this so contract addresses match with cw20 test cases
    let _ = instantiate_cw20_base_default(&mut app);

    let DefaultTestSetup {
        core_addr: _,
        proposal_single: _,
        pre_propose,
        approver_core_addr,
        proposal_single_approver: _,
        pre_propose_approver,
    } = setup_default_test(
        &mut app,
        Some(UncheckedDepositInfo {
            denom: DepositToken::Token {
                denom: UncheckedDenom::Native("ujuno".to_string()),
            },
            amount: Uint128::new(10),
            refund_policy: DepositRefundPolicy::Always,
        }),
        false,
    );

    // Ensure approver is set to the pre_propose_approver
    let approver: Addr = app
        .wrap()
        .query_wasm_smart(
            pre_propose.clone(),
            &QueryMsg::QueryExtension {
                msg: QueryExt::Approver {},
            },
        )
        .unwrap();
    assert_eq!(approver, pre_propose_approver);

    // Fail to change approver by non-approver.
    let err: PreProposeError = app
        .execute_contract(
            Addr::unchecked("someone"),
            pre_propose.clone(),
            &ExecuteMsg::Extension {
                msg: ExecuteExt::UpdateApprover {
                    address: "someone".to_string(),
                },
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, PreProposeError::Unauthorized {});

    // Fail to reset approver back to approver DAO by non-approver.
    let err: PreProposeError = app
        .execute_contract(
            Addr::unchecked("someone"),
            pre_propose_approver.clone(),
            &ApproverExecuteMsg::Extension {
                msg: ApproverExecuteExt::ResetApprover {},
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, PreProposeError::Unauthorized {});

    // Reset approver back to approver DAO.
    app.execute_contract(
        approver_core_addr.clone(),
        pre_propose_approver.clone(),
        &ApproverExecuteMsg::Extension {
            msg: ApproverExecuteExt::ResetApprover {},
        },
        &[],
    )
    .unwrap();

    // Ensure approver is reset back to the approver DAO
    let approver: Addr = app
        .wrap()
        .query_wasm_smart(
            pre_propose.clone(),
            &QueryMsg::QueryExtension {
                msg: QueryExt::Approver {},
            },
        )
        .unwrap();
    assert_eq!(approver, approver_core_addr);
}
