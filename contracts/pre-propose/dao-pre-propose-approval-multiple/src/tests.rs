use cosmwasm_std::{coins, from_json, to_json_binary, Addr, Coin, Empty, Uint128};
use cw2::ContractVersion;
use cw20::Cw20Coin;
use cw_denom::UncheckedDenom;
use cw_multi_test::{App, BankSudo, Contract, ContractWrapper, Executor};
use cw_utils::Duration;
use dao_interface::proposal::InfoResponse;
use dao_interface::state::ProposalModule;
use dao_interface::state::{Admin, ModuleInstantiateInfo};
use dao_pre_propose_base::{error::PreProposeError, msg::DepositInfoResponse, state::Config};
use dao_proposal_multiple::query::ProposalResponse;
use dao_testing::helpers::instantiate_with_cw4_groups_governance;
use dao_voting::multiple_choice::{
    MultipleChoiceOption, MultipleChoiceOptions, MultipleChoiceVote, VotingStrategy,
};
use dao_voting::pre_propose::{PreProposeSubmissionPolicy, PreProposeSubmissionPolicyError};
use dao_voting::{
    approval::ApprovalProposalStatus,
    deposit::{CheckedDepositInfo, DepositRefundPolicy, DepositToken, UncheckedDepositInfo},
    pre_propose::{PreProposeInfo, ProposalCreationPolicy},
    status::Status,
    threshold::PercentageThreshold,
};

use crate::state::Proposal;
use crate::{contract::*, msg::*};

fn dao_proposal_multiple_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        dao_proposal_multiple::contract::execute,
        dao_proposal_multiple::contract::instantiate,
        dao_proposal_multiple::contract::query,
    )
    .with_migrate(dao_proposal_multiple::contract::migrate)
    .with_reply(dao_proposal_multiple::contract::reply);
    Box::new(contract)
}

fn dao_pre_propose_approval_multiple_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(execute, instantiate, query).with_migrate(migrate);
    Box::new(contract)
}

fn cw20_base_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw20_base::contract::execute,
        cw20_base::contract::instantiate,
        cw20_base::contract::query,
    );
    Box::new(contract)
}

fn get_default_proposal_module_instantiate(
    app: &mut App,
    deposit_info: Option<UncheckedDepositInfo>,
    open_proposal_submission: bool,
) -> dao_proposal_multiple::msg::InstantiateMsg {
    let pre_propose_id = app.store_code(dao_pre_propose_approval_multiple_contract());

    let submission_policy = if open_proposal_submission {
        PreProposeSubmissionPolicy::Anyone { denylist: vec![] }
    } else {
        PreProposeSubmissionPolicy::Specific {
            dao_members: true,
            allowlist: vec![],
            denylist: vec![],
        }
    };

    dao_proposal_multiple::msg::InstantiateMsg {
        voting_strategy: VotingStrategy::SingleChoice {
            quorum: PercentageThreshold::Majority {},
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
                        approver: "approver".to_string(),
                    },
                })
                .unwrap(),
                admin: Some(Admin::CoreModule {}),
                funds: vec![],
                label: "baby's first pre-propose module".to_string(),
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
    proposal_multiple: Addr,
    pre_propose: Addr,
}

fn setup_default_test(
    app: &mut App,
    deposit_info: Option<UncheckedDepositInfo>,
    open_proposal_submission: bool,
) -> DefaultTestSetup {
    let dao_proposal_multiple_id = app.store_code(dao_proposal_multiple_contract());

    let proposal_module_instantiate =
        get_default_proposal_module_instantiate(app, deposit_info, open_proposal_submission);

    let core_addr = instantiate_with_cw4_groups_governance(
        app,
        dao_proposal_multiple_id,
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

    assert_eq!(proposal_modules.len(), 1);
    let proposal_multiple = proposal_modules.into_iter().next().unwrap().address;
    let proposal_creation_policy = app
        .wrap()
        .query_wasm_smart(
            proposal_multiple.clone(),
            &dao_proposal_multiple::msg::QueryMsg::ProposalCreationPolicy {},
        )
        .unwrap();

    let pre_propose = match proposal_creation_policy {
        ProposalCreationPolicy::Module { addr } => addr,
        _ => panic!("expected a module for the proposal creation policy"),
    };

    // Make sure things were set up correctly.
    assert_eq!(
        proposal_multiple,
        get_proposal_module(app, pre_propose.clone())
    );
    assert_eq!(core_addr, get_dao(app, pre_propose.clone()));
    assert_eq!(
        InfoResponse {
            info: ContractVersion {
                contract: "crates.io:dao-pre-propose-approval-multiple".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string()
            }
        },
        get_info(app, pre_propose.clone())
    );

    DefaultTestSetup {
        core_addr,
        proposal_multiple,
        pre_propose,
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
                choices: MultipleChoiceOptions {
                    options: vec![
                        MultipleChoiceOption {
                            title: "A".to_string(),
                            description: "A".to_string(),
                            msgs: vec![],
                        },
                        MultipleChoiceOption {
                            title: "B".to_string(),
                            description: "B".to_string(),
                            msgs: vec![],
                        },
                    ],
                },
                vote: None,
            },
        },
        funds,
    )
    .unwrap();

    // Query for pending proposal and return latest id.
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

    // Return last item in ascending list, id is first element of tuple
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

fn vote(app: &mut App, module: Addr, sender: &str, id: u64, option_id: u32) -> Status {
    app.execute_contract(
        Addr::unchecked(sender),
        module.clone(),
        &dao_proposal_multiple::msg::ExecuteMsg::Vote {
            proposal_id: id,
            vote: MultipleChoiceVote { option_id },
            rationale: None,
        },
        &[],
    )
    .unwrap();

    let proposal: ProposalResponse = app
        .wrap()
        .query_wasm_smart(
            module,
            &dao_proposal_multiple::msg::QueryMsg::Proposal { proposal_id: id },
        )
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
        &dao_proposal_multiple::msg::ExecuteMsg::Close { proposal_id },
        &[],
    )
    .unwrap();
}

fn execute_proposal(app: &mut App, module: Addr, sender: &str, proposal_id: u64) {
    app.execute_contract(
        Addr::unchecked(sender),
        module,
        &dao_proposal_multiple::msg::ExecuteMsg::Execute { proposal_id },
        &[],
    )
    .unwrap();
}

fn approve_proposal(app: &mut App, module: Addr, sender: &str, proposal_id: u64) -> u64 {
    let res = app
        .execute_contract(
            Addr::unchecked(sender),
            module,
            &ExecuteMsg::Extension {
                msg: ExecuteExt::Approve { id: proposal_id },
            },
            &[],
        )
        .unwrap();

    // Parse attrs from approve_proposal response
    let attrs = res.custom_attrs(res.events.len() - 1);
    // Return ID
    attrs[attrs.len() - 2].value.parse().unwrap()
}

fn reject_proposal(app: &mut App, module: Addr, sender: &str, proposal_id: u64) {
    app.execute_contract(
        Addr::unchecked(sender),
        module,
        &ExecuteMsg::Extension {
            msg: ExecuteExt::Reject { id: proposal_id },
        },
        &[],
    )
    .unwrap();
}

enum ApprovalStatus {
    Approved,
    Rejected,
}

enum EndStatus {
    PassedA,
    PassedB,
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

    let DefaultTestSetup {
        core_addr,
        proposal_multiple,
        pre_propose,
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
    let pre_propose_id =
        make_pre_proposal(&mut app, pre_propose.clone(), "ekez", &coins(10, "ujuno"));

    // Make sure it went away.
    let balance = get_balance_native(&app, "ekez", "ujuno");
    assert_eq!(balance, Uint128::zero());

    // Approver approves or rejects proposal
    match approval_status {
        ApprovalStatus::Approved => {
            // Approver approves, new proposal id is returned
            let id = approve_proposal(&mut app, pre_propose, "approver", pre_propose_id);

            // Voting happens on newly created proposal
            #[allow(clippy::type_complexity)]
            let (position, expected_status, trigger_refund): (
                _,
                _,
                fn(&mut App, Addr, &str, u64) -> (),
            ) = match end_status {
                EndStatus::PassedA => (0, Status::Passed, execute_proposal),
                EndStatus::PassedB => (1, Status::Passed, execute_proposal),
                EndStatus::Failed => (2, Status::Rejected, close_proposal),
            };
            let new_status = vote(&mut app, proposal_multiple.clone(), "ekez", id, position);
            assert_eq!(new_status, expected_status);

            // Close or execute the proposal to trigger a refund.
            trigger_refund(&mut app, proposal_multiple, "ekez", id);
        }
        ApprovalStatus::Rejected => {
            // Proposal is rejected by approver
            // No proposal is created so there is no voting
            reject_proposal(&mut app, pre_propose, "approver", pre_propose_id);
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
        proposal_multiple,
        pre_propose,
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
    let pre_propose_id = make_pre_proposal(&mut app, pre_propose.clone(), "ekez", &[]);

    // Make sure it went await.
    let balance = get_balance_cw20(&app, cw20_address.clone(), "ekez");
    assert_eq!(balance, Uint128::zero());

    // Approver approves or rejects proposal
    match approval_status {
        ApprovalStatus::Approved => {
            // Approver approves, new proposal id is returned
            let id = approve_proposal(&mut app, pre_propose.clone(), "approver", pre_propose_id);

            // Voting happens on newly created proposal
            #[allow(clippy::type_complexity)]
            let (position, expected_status, trigger_refund): (
                _,
                _,
                fn(&mut App, Addr, &str, u64) -> (),
            ) = match end_status {
                EndStatus::PassedA => (0, Status::Passed, execute_proposal),
                EndStatus::PassedB => (1, Status::Passed, execute_proposal),
                EndStatus::Failed => (2, Status::Rejected, close_proposal),
            };
            let new_status = vote(&mut app, proposal_multiple.clone(), "ekez", id, position);
            assert_eq!(new_status, expected_status);

            // Close or execute the proposal to trigger a refund.
            trigger_refund(&mut app, proposal_multiple, "ekez", id);
        }
        ApprovalStatus::Rejected => {
            // Proposal is rejected by approver
            // No proposal is created so there is no voting
            reject_proposal(&mut app, pre_propose.clone(), "approver", pre_propose_id);
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
        EndStatus::PassedA,
        DepositRefundPolicy::Always,
        RefundReceiver::Proposer,
        ApprovalStatus::Approved,
    )
}

#[test]
fn test_cw20_passed_always_refund() {
    test_cw20_permutation(
        EndStatus::PassedB,
        DepositRefundPolicy::Always,
        RefundReceiver::Proposer,
        ApprovalStatus::Approved,
    )
}

#[test]
fn test_native_passed_never_refund() {
    test_native_permutation(
        EndStatus::PassedB,
        DepositRefundPolicy::Never,
        RefundReceiver::Dao,
        ApprovalStatus::Approved,
    )
}

#[test]
fn test_cw20_passed_never_refund() {
    test_cw20_permutation(
        EndStatus::PassedA,
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
        EndStatus::PassedA,
        DepositRefundPolicy::OnlyPassed,
        RefundReceiver::Proposer,
        ApprovalStatus::Approved,
    )
}
#[test]
fn test_cw20_passed_passed_refund() {
    test_cw20_permutation(
        EndStatus::PassedA,
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

    let DefaultTestSetup {
        core_addr: _,
        proposal_multiple,
        pre_propose,
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
    let first_pre_propose_id =
        make_pre_proposal(&mut app, pre_propose.clone(), "ekez", &coins(10, "ujuno"));
    let balance = get_balance_native(&app, "ekez", "ujuno");
    assert_eq!(10, balance.u128());

    // Approver approves prop, balance remains the same
    let first_id = approve_proposal(
        &mut app,
        pre_propose.clone(),
        "approver",
        first_pre_propose_id,
    );
    let balance = get_balance_native(&app, "ekez", "ujuno");
    assert_eq!(10, balance.u128());

    let second_pre_propose_id =
        make_pre_proposal(&mut app, pre_propose.clone(), "ekez", &coins(10, "ujuno"));
    let balance = get_balance_native(&app, "ekez", "ujuno");
    assert_eq!(0, balance.u128());

    // Approver approves prop, balance remains the same
    let second_id = approve_proposal(&mut app, pre_propose, "approver", second_pre_propose_id);
    let balance = get_balance_native(&app, "ekez", "ujuno");
    assert_eq!(0, balance.u128());

    // Finish up the first proposal.
    let new_status = vote(&mut app, proposal_multiple.clone(), "ekez", first_id, 0);
    assert_eq!(Status::Passed, new_status);

    // Still zero.
    let balance = get_balance_native(&app, "ekez", "ujuno");
    assert_eq!(0, balance.u128());

    execute_proposal(&mut app, proposal_multiple.clone(), "ekez", first_id);

    // First proposal refunded.
    let balance = get_balance_native(&app, "ekez", "ujuno");
    assert_eq!(10, balance.u128());

    // Finish up the second proposal.
    let new_status = vote(&mut app, proposal_multiple.clone(), "ekez", second_id, 2);
    assert_eq!(Status::Rejected, new_status);

    // Still zero.
    let balance = get_balance_native(&app, "ekez", "ujuno");
    assert_eq!(10, balance.u128());

    close_proposal(&mut app, proposal_multiple, "ekez", second_id);

    // All deposits have been refunded.
    let balance = get_balance_native(&app, "ekez", "ujuno");
    assert_eq!(20, balance.u128());
}

#[test]
fn test_pending_proposal_queries() {
    let mut app = App::default();

    let DefaultTestSetup {
        core_addr: _,
        proposal_multiple: _,
        pre_propose,
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
    make_pre_proposal(&mut app, pre_propose.clone(), "ekez", &coins(10, "ujuno"));
    make_pre_proposal(&mut app, pre_propose.clone(), "ekez", &coins(10, "ujuno"));

    // Query for individual proposal
    let prop1: Proposal = app
        .wrap()
        .query_wasm_smart(
            pre_propose.clone(),
            &QueryMsg::QueryExtension {
                msg: QueryExt::PendingProposal { id: 1 },
            },
        )
        .unwrap();
    assert_eq!(prop1.approval_id, 1);
    assert_eq!(prop1.status, ApprovalProposalStatus::Pending {});

    let prop1: Proposal = app
        .wrap()
        .query_wasm_smart(
            pre_propose.clone(),
            &QueryMsg::QueryExtension {
                msg: QueryExt::Proposal { id: 1 },
            },
        )
        .unwrap();
    assert_eq!(prop1.approval_id, 1);
    assert_eq!(prop1.status, ApprovalProposalStatus::Pending {});

    // Query for the pre-propose proposals
    let pre_propose_props: Vec<Proposal> = app
        .wrap()
        .query_wasm_smart(
            pre_propose.clone(),
            &QueryMsg::QueryExtension {
                msg: QueryExt::PendingProposals {
                    start_after: None,
                    limit: None,
                },
            },
        )
        .unwrap();
    assert_eq!(pre_propose_props.len(), 2);
    assert_eq!(pre_propose_props[0].approval_id, 1);

    // Query props in reverse
    let reverse_pre_propose_props: Vec<Proposal> = app
        .wrap()
        .query_wasm_smart(
            pre_propose,
            &QueryMsg::QueryExtension {
                msg: QueryExt::ReversePendingProposals {
                    start_before: None,
                    limit: None,
                },
            },
        )
        .unwrap();

    assert_eq!(reverse_pre_propose_props.len(), 2);
    assert_eq!(reverse_pre_propose_props[0].approval_id, 2);
}

#[test]
fn test_completed_proposal_queries() {
    let mut app = App::default();

    let DefaultTestSetup {
        core_addr: _,
        proposal_multiple: _,
        pre_propose,
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
    let approve_id = make_pre_proposal(&mut app, pre_propose.clone(), "ekez", &coins(10, "ujuno"));
    let reject_id = make_pre_proposal(&mut app, pre_propose.clone(), "ekez", &coins(10, "ujuno"));

    let is_pending: bool = app
        .wrap()
        .query_wasm_smart(
            pre_propose.clone(),
            &QueryMsg::QueryExtension {
                msg: QueryExt::IsPending { id: approve_id },
            },
        )
        .unwrap();
    assert!(is_pending);

    let created_approved_id =
        approve_proposal(&mut app, pre_propose.clone(), "approver", approve_id);
    reject_proposal(&mut app, pre_propose.clone(), "approver", reject_id);

    let is_pending: bool = app
        .wrap()
        .query_wasm_smart(
            pre_propose.clone(),
            &QueryMsg::QueryExtension {
                msg: QueryExt::IsPending { id: approve_id },
            },
        )
        .unwrap();
    assert!(!is_pending);

    // Query for individual proposals
    let prop1: Proposal = app
        .wrap()
        .query_wasm_smart(
            pre_propose.clone(),
            &QueryMsg::QueryExtension {
                msg: QueryExt::CompletedProposal { id: approve_id },
            },
        )
        .unwrap();
    assert_eq!(
        prop1.status,
        ApprovalProposalStatus::Approved {
            created_proposal_id: created_approved_id
        }
    );
    let prop1: Proposal = app
        .wrap()
        .query_wasm_smart(
            pre_propose.clone(),
            &QueryMsg::QueryExtension {
                msg: QueryExt::Proposal { id: approve_id },
            },
        )
        .unwrap();
    assert_eq!(
        prop1.status,
        ApprovalProposalStatus::Approved {
            created_proposal_id: created_approved_id
        }
    );

    let prop1_id: Option<u64> = app
        .wrap()
        .query_wasm_smart(
            pre_propose.clone(),
            &QueryMsg::QueryExtension {
                msg: QueryExt::CompletedProposalIdForCreatedProposalId {
                    id: created_approved_id,
                },
            },
        )
        .unwrap();
    assert_eq!(prop1_id, Some(approve_id));

    let prop2: Proposal = app
        .wrap()
        .query_wasm_smart(
            pre_propose.clone(),
            &QueryMsg::QueryExtension {
                msg: QueryExt::CompletedProposal { id: reject_id },
            },
        )
        .unwrap();
    assert_eq!(prop2.status, ApprovalProposalStatus::Rejected {});

    // Query for the pre-propose proposals
    let pre_propose_props: Vec<Proposal> = app
        .wrap()
        .query_wasm_smart(
            pre_propose.clone(),
            &QueryMsg::QueryExtension {
                msg: QueryExt::CompletedProposals {
                    start_after: None,
                    limit: None,
                },
            },
        )
        .unwrap();
    assert_eq!(pre_propose_props.len(), 2);
    assert_eq!(pre_propose_props[0].approval_id, approve_id);
    assert_eq!(pre_propose_props[1].approval_id, reject_id);

    // Query props in reverse
    let reverse_pre_propose_props: Vec<Proposal> = app
        .wrap()
        .query_wasm_smart(
            pre_propose,
            &QueryMsg::QueryExtension {
                msg: QueryExt::ReverseCompletedProposals {
                    start_before: None,
                    limit: None,
                },
            },
        )
        .unwrap();

    assert_eq!(reverse_pre_propose_props.len(), 2);
    assert_eq!(reverse_pre_propose_props[0].approval_id, reject_id);
    assert_eq!(reverse_pre_propose_props[1].approval_id, approve_id);
}

#[test]
fn test_set_version() {
    let mut app = App::default();

    let DefaultTestSetup {
        core_addr: _,
        proposal_multiple: _,
        pre_propose,
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
            .query_wasm_raw(pre_propose, "contract_info".as_bytes())
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

    let DefaultTestSetup {
        core_addr,
        proposal_multiple: _,
        pre_propose,
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
                    choices: MultipleChoiceOptions {
                        options: vec![
                            MultipleChoiceOption {
                                title: "A".to_string(),
                                description: "A".to_string(),
                                msgs: vec![],
                            },
                            MultipleChoiceOption {
                                title: "B".to_string(),
                                description: "B".to_string(),
                                msgs: vec![],
                            },
                        ],
                    },
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
    let DefaultTestSetup {
        core_addr: _,
        proposal_multiple: _,
        pre_propose,
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

    // Only approver can approve
    let err: PreProposeError = app
        .execute_contract(
            Addr::unchecked("nonapprover"),
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

    // Only approver can reject
    let err: PreProposeError = app
        .execute_contract(
            Addr::unchecked("nonapprover"),
            pre_propose.clone(),
            &ExecuteMsg::Extension {
                msg: ExecuteExt::Reject { id: pre_propose_id },
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, PreProposeError::Unauthorized {});

    // Updating approver after proposal created does not change old proposal's
    // approver
    app.execute_contract(
        Addr::unchecked("approver"),
        pre_propose.clone(),
        &ExecuteMsg::Extension {
            msg: ExecuteExt::UpdateApprover {
                address: "newapprover".to_string(),
            },
        },
        &[],
    )
    .unwrap();

    let err: PreProposeError = app
        .execute_contract(
            Addr::unchecked("newapprover"),
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

    // Old approver can still approve.
    app.execute_contract(
        Addr::unchecked("approver"),
        pre_propose.clone(),
        &ExecuteMsg::Extension {
            msg: ExecuteExt::Approve { id: pre_propose_id },
        },
        &[],
    )
    .unwrap();

    // Non-member proposes.
    mint_natives(&mut app, "nonmember", coins(10, "ujuno"));
    let pre_propose_id = make_pre_proposal(
        &mut app,
        pre_propose.clone(),
        "nonmember",
        &coins(10, "ujuno"),
    );

    // Old approver cannot approve nor reject.
    let err: PreProposeError = app
        .execute_contract(
            Addr::unchecked("approver"),
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

    let err: PreProposeError = app
        .execute_contract(
            Addr::unchecked("approver"),
            pre_propose.clone(),
            &ExecuteMsg::Extension {
                msg: ExecuteExt::Reject { id: pre_propose_id },
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, PreProposeError::Unauthorized {});

    // New approver can now approve.
    app.execute_contract(
        Addr::unchecked("newapprover"),
        pre_propose.clone(),
        &ExecuteMsg::Extension {
            msg: ExecuteExt::Approve { id: pre_propose_id },
        },
        &[],
    )
    .unwrap();
}

#[test]
fn test_propose_open_proposal_submission() {
    let mut app = App::default();
    let DefaultTestSetup {
        core_addr: _,
        proposal_multiple,
        pre_propose,
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

    // Approver approves
    let id = approve_proposal(&mut app, pre_propose, "approver", pre_propose_id);

    // Member votes.
    let new_status = vote(&mut app, proposal_multiple, "ekez", id, 0);
    assert_eq!(Status::Passed, new_status)
}

#[test]
fn test_no_deposit_required_open_submission() {
    let mut app = App::default();
    let DefaultTestSetup {
        core_addr: _,
        proposal_multiple,
        pre_propose,
    } = setup_default_test(
        &mut app, None, true, // yes, open proposal submission.
    );

    // Non-member proposes.
    let pre_propose_id = make_pre_proposal(&mut app, pre_propose.clone(), "nonmember", &[]);

    // Approver approves
    let id = approve_proposal(&mut app, pre_propose, "approver", pre_propose_id);

    // Member votes.
    let new_status = vote(&mut app, proposal_multiple, "ekez", id, 0);
    assert_eq!(Status::Passed, new_status)
}

#[test]
fn test_no_deposit_required_members_submission() {
    let mut app = App::default();
    let DefaultTestSetup {
        core_addr: _,
        proposal_multiple,
        pre_propose,
    } = setup_default_test(
        &mut app, None, false, // no open proposal submission.
    );

    // Non-member proposes and this fails.
    let err: PreProposeError = app
        .execute_contract(
            Addr::unchecked("nonmember"),
            pre_propose.clone(),
            &ExecuteMsg::Propose {
                msg: ProposeMessage::Propose {
                    title: "I would like to join the DAO".to_string(),
                    description: "though, I am currently not a member.".to_string(),
                    choices: MultipleChoiceOptions {
                        options: vec![
                            MultipleChoiceOption {
                                title: "A".to_string(),
                                description: "A".to_string(),
                                msgs: vec![],
                            },
                            MultipleChoiceOption {
                                title: "B".to_string(),
                                description: "B".to_string(),
                                msgs: vec![],
                            },
                        ],
                    },
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

    let pre_propose_id = make_pre_proposal(&mut app, pre_propose.clone(), "ekez", &[]);

    // Approver approves
    let id = approve_proposal(&mut app, pre_propose, "approver", pre_propose_id);

    let new_status = vote(&mut app, proposal_multiple, "ekez", id, 0);
    assert_eq!(Status::Passed, new_status)
}

#[test]
fn test_anyone_denylist() {
    let mut app = App::default();
    let DefaultTestSetup {
        core_addr,
        pre_propose,
        ..
    } = setup_default_test(&mut app, None, false);

    update_config(
        &mut app,
        pre_propose.clone(),
        core_addr.as_str(),
        None,
        PreProposeSubmissionPolicy::Anyone { denylist: vec![] },
    );

    let rando = "rando";

    // Proposal succeeds when anyone can propose.
    assert!(query_can_propose(&app, pre_propose.clone(), rando));
    make_pre_proposal(&mut app, pre_propose.clone(), rando, &[]);

    update_config(
        &mut app,
        pre_propose.clone(),
        core_addr.as_str(),
        None,
        PreProposeSubmissionPolicy::Anyone {
            denylist: vec![Addr::unchecked(rando)],
        },
    );

    // Proposing fails if on denylist.
    assert!(!query_can_propose(&app, pre_propose.clone(), rando));
    let err: PreProposeError = app
        .execute_contract(
            Addr::unchecked(rando),
            pre_propose.clone(),
            &ExecuteMsg::Propose {
                msg: ProposeMessage::Propose {
                    title: "I would like to join the DAO".to_string(),
                    description: "though, I am currently not a member.".to_string(),
                    choices: MultipleChoiceOptions {
                        options: vec![
                            MultipleChoiceOption {
                                title: "A".to_string(),
                                description: "A".to_string(),
                                msgs: vec![],
                            },
                            MultipleChoiceOption {
                                title: "B".to_string(),
                                description: "B".to_string(),
                                msgs: vec![],
                            },
                        ],
                    },
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

    // Proposing succeeds if not on denylist.
    assert!(query_can_propose(&app, pre_propose.clone(), "ekez"));
    make_pre_proposal(&mut app, pre_propose, "ekez", &[]);
}

#[test]
fn test_specific_allowlist_denylist() {
    let mut app = App::default();
    let DefaultTestSetup {
        core_addr,
        pre_propose,
        ..
    } = setup_default_test(&mut app, None, false);

    update_config(
        &mut app,
        pre_propose.clone(),
        core_addr.as_str(),
        None,
        PreProposeSubmissionPolicy::Specific {
            dao_members: true,
            allowlist: vec![],
            denylist: vec![],
        },
    );

    // Proposal succeeds for member.
    assert!(query_can_propose(&app, pre_propose.clone(), "ekez"));
    make_pre_proposal(&mut app, pre_propose.clone(), "ekez", &[]);

    let rando = "rando";

    // Proposing fails for non-member.
    assert!(!query_can_propose(&app, pre_propose.clone(), rando));
    let err: PreProposeError = app
        .execute_contract(
            Addr::unchecked(rando),
            pre_propose.clone(),
            &ExecuteMsg::Propose {
                msg: ProposeMessage::Propose {
                    title: "I would like to join the DAO".to_string(),
                    description: "though, I am currently not a member.".to_string(),
                    choices: MultipleChoiceOptions {
                        options: vec![
                            MultipleChoiceOption {
                                title: "A".to_string(),
                                description: "A".to_string(),
                                msgs: vec![],
                            },
                            MultipleChoiceOption {
                                title: "B".to_string(),
                                description: "B".to_string(),
                                msgs: vec![],
                            },
                        ],
                    },
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

    update_config(
        &mut app,
        pre_propose.clone(),
        core_addr.as_str(),
        None,
        PreProposeSubmissionPolicy::Specific {
            dao_members: true,
            allowlist: vec![Addr::unchecked(rando)],
            denylist: vec![],
        },
    );

    // Proposal succeeds if on allowlist.
    assert!(query_can_propose(&app, pre_propose.clone(), rando));
    make_pre_proposal(&mut app, pre_propose.clone(), rando, &[]);

    update_config(
        &mut app,
        pre_propose.clone(),
        core_addr.as_str(),
        None,
        PreProposeSubmissionPolicy::Specific {
            dao_members: true,
            allowlist: vec![Addr::unchecked(rando)],
            denylist: vec![Addr::unchecked("ekez")],
        },
    );

    // Proposing fails if on denylist.
    assert!(!query_can_propose(&app, pre_propose.clone(), "ekez"));
    let err: PreProposeError = app
        .execute_contract(
            Addr::unchecked("ekez"),
            pre_propose.clone(),
            &ExecuteMsg::Propose {
                msg: ProposeMessage::Propose {
                    title: "Let me propose!".to_string(),
                    description: "I am a member!!!".to_string(),
                    choices: MultipleChoiceOptions {
                        options: vec![
                            MultipleChoiceOption {
                                title: "A".to_string(),
                                description: "A".to_string(),
                                msgs: vec![],
                            },
                            MultipleChoiceOption {
                                title: "B".to_string(),
                                description: "B".to_string(),
                                msgs: vec![],
                            },
                        ],
                    },
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

    update_config(
        &mut app,
        pre_propose.clone(),
        core_addr.as_str(),
        None,
        PreProposeSubmissionPolicy::Specific {
            dao_members: false,
            allowlist: vec![Addr::unchecked(rando)],
            denylist: vec![],
        },
    );

    // Proposing fails if members not allowed.
    assert!(!query_can_propose(&app, pre_propose.clone(), "ekez"));
    let err: PreProposeError = app
        .execute_contract(
            Addr::unchecked("ekez"),
            pre_propose.clone(),
            &ExecuteMsg::Propose {
                msg: ProposeMessage::Propose {
                    title: "Let me propose!".to_string(),
                    description: "I am a member!!!".to_string(),
                    choices: MultipleChoiceOptions {
                        options: vec![
                            MultipleChoiceOption {
                                title: "A".to_string(),
                                description: "A".to_string(),
                                msgs: vec![],
                            },
                            MultipleChoiceOption {
                                title: "B".to_string(),
                                description: "B".to_string(),
                                msgs: vec![],
                            },
                        ],
                    },
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

    // Proposal succeeds if on allowlist.
    assert!(query_can_propose(&app, pre_propose.clone(), rando));
    make_pre_proposal(&mut app, pre_propose.clone(), rando, &[]);
}

#[test]
#[should_panic(expected = "invalid zero deposit. set the deposit to `None` to have no deposit")]
fn test_instantiate_with_zero_native_deposit() {
    let mut app = App::default();

    let dao_proposal_multiple_id = app.store_code(dao_proposal_multiple_contract());

    let proposal_module_instantiate = {
        let pre_propose_id = app.store_code(dao_pre_propose_approval_multiple_contract());

        dao_proposal_multiple::msg::InstantiateMsg {
            voting_strategy: VotingStrategy::SingleChoice {
                quorum: PercentageThreshold::Majority {},
            },
            max_voting_period: Duration::Time(86400),
            min_voting_period: None,
            only_members_execute: false,
            allow_revoting: false,
            pre_propose_info: PreProposeInfo::ModuleMayPropose {
                info: ModuleInstantiateInfo {
                    code_id: pre_propose_id,
                    msg: to_json_binary(&InstantiateMsg {
                        deposit_info: Some(UncheckedDepositInfo {
                            denom: DepositToken::Token {
                                denom: UncheckedDenom::Native("ujuno".to_string()),
                            },
                            amount: Uint128::zero(),
                            refund_policy: DepositRefundPolicy::OnlyPassed,
                        }),
                        submission_policy: PreProposeSubmissionPolicy::Specific {
                            dao_members: true,
                            allowlist: vec![],
                            denylist: vec![],
                        },
                        extension: InstantiateExt {
                            approver: "approver".to_string(),
                        },
                    })
                    .unwrap(),
                    admin: Some(Admin::CoreModule {}),
                    funds: vec![],
                    label: "baby's first pre-propose module".to_string(),
                },
            },
            close_proposal_on_execution_failure: false,
            veto: None,
        }
    };

    // Should panic.
    instantiate_with_cw4_groups_governance(
        &mut app,
        dao_proposal_multiple_id,
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
}

#[test]
#[should_panic(expected = "invalid zero deposit. set the deposit to `None` to have no deposit")]
fn test_instantiate_with_zero_cw20_deposit() {
    let mut app = App::default();

    let cw20_addr = instantiate_cw20_base_default(&mut app);

    let dao_proposal_multiple_id = app.store_code(dao_proposal_multiple_contract());

    let proposal_module_instantiate = {
        let pre_propose_id = app.store_code(dao_pre_propose_approval_multiple_contract());

        dao_proposal_multiple::msg::InstantiateMsg {
            voting_strategy: VotingStrategy::SingleChoice {
                quorum: PercentageThreshold::Majority {},
            },
            max_voting_period: Duration::Time(86400),
            min_voting_period: None,
            only_members_execute: false,
            allow_revoting: false,
            pre_propose_info: PreProposeInfo::ModuleMayPropose {
                info: ModuleInstantiateInfo {
                    code_id: pre_propose_id,
                    msg: to_json_binary(&InstantiateMsg {
                        deposit_info: Some(UncheckedDepositInfo {
                            denom: DepositToken::Token {
                                denom: UncheckedDenom::Cw20(cw20_addr.into_string()),
                            },
                            amount: Uint128::zero(),
                            refund_policy: DepositRefundPolicy::OnlyPassed,
                        }),
                        submission_policy: PreProposeSubmissionPolicy::Specific {
                            dao_members: true,
                            allowlist: vec![],
                            denylist: vec![],
                        },
                        extension: InstantiateExt {
                            approver: "approver".to_string(),
                        },
                    })
                    .unwrap(),
                    admin: Some(Admin::CoreModule {}),
                    funds: vec![],
                    label: "baby's first pre-propose module".to_string(),
                },
            },
            close_proposal_on_execution_failure: false,
            veto: None,
        }
    };

    // Should panic.
    instantiate_with_cw4_groups_governance(
        &mut app,
        dao_proposal_multiple_id,
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
}

#[test]
fn test_update_config() {
    let mut app = App::default();
    let DefaultTestSetup {
        core_addr,
        proposal_multiple,
        pre_propose,
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

    let pre_propose_id = make_pre_proposal(&mut app, pre_propose.clone(), "ekez", &[]);

    // Approver approves
    let id = approve_proposal(&mut app, pre_propose.clone(), "approver", pre_propose_id);

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
    let new_pre_propose_id =
        make_pre_proposal(&mut app, pre_propose.clone(), "ekez", &coins(10, "ujuno"));

    // Approver approves
    let new_id = approve_proposal(
        &mut app,
        pre_propose.clone(),
        "approver",
        new_pre_propose_id,
    );

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
    vote(&mut app, proposal_multiple.clone(), "ekez", id, 0);
    vote(&mut app, proposal_multiple.clone(), "ekez", new_id, 0);
    execute_proposal(&mut app, proposal_multiple.clone(), "ekez", id);
    execute_proposal(&mut app, proposal_multiple.clone(), "ekez", new_id);
    // Deposit should not have been refunded (never policy in use).
    let balance = get_balance_native(&app, "ekez", "ujuno");
    assert_eq!(balance, Uint128::new(0));

    // Only the core module can update the config.
    let err = update_config_should_fail(
        &mut app,
        pre_propose.clone(),
        proposal_multiple.as_str(),
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
fn test_update_submission_policy() {
    let mut app = App::default();
    let DefaultTestSetup {
        core_addr,
        pre_propose,
        ..
    } = setup_default_test(&mut app, None, true);

    let config = get_config(&app, pre_propose.clone());
    assert_eq!(
        config,
        Config {
            deposit_info: None,
            submission_policy: PreProposeSubmissionPolicy::Anyone { denylist: vec![] },
        }
    );

    // Only the core module can update the submission policy.
    let err: PreProposeError = app
        .execute_contract(
            Addr::unchecked("ekez"),
            pre_propose.clone(),
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
    assert_eq!(err, PreProposeError::NotDao {});

    // Append to denylist, with auto de-dupe.
    app.execute_contract(
        core_addr.clone(),
        pre_propose.clone(),
        &ExecuteMsg::UpdateSubmissionPolicy {
            denylist_add: Some(vec!["ekez".to_string(), "ekez".to_string()]),
            denylist_remove: None,
            set_dao_members: None,
            allowlist_add: None,
            allowlist_remove: None,
        },
        &[],
    )
    .unwrap();

    let config = get_config(&app, pre_propose.clone());
    assert_eq!(
        config,
        Config {
            deposit_info: None,
            submission_policy: PreProposeSubmissionPolicy::Anyone {
                denylist: vec![Addr::unchecked("ekez")],
            },
        }
    );

    // Add and remove to/from denylist.
    app.execute_contract(
        core_addr.clone(),
        pre_propose.clone(),
        &ExecuteMsg::UpdateSubmissionPolicy {
            denylist_add: Some(vec!["someone".to_string(), "else".to_string()]),
            denylist_remove: Some(vec!["ekez".to_string()]),
            set_dao_members: None,
            allowlist_add: None,
            allowlist_remove: None,
        },
        &[],
    )
    .unwrap();

    let config = get_config(&app, pre_propose.clone());
    assert_eq!(
        config,
        Config {
            deposit_info: None,
            submission_policy: PreProposeSubmissionPolicy::Anyone {
                denylist: vec![Addr::unchecked("else"), Addr::unchecked("someone")],
            },
        }
    );

    // Remove from denylist.
    app.execute_contract(
        core_addr.clone(),
        pre_propose.clone(),
        &ExecuteMsg::UpdateSubmissionPolicy {
            denylist_add: None,
            denylist_remove: Some(vec!["someone".to_string(), "else".to_string()]),
            set_dao_members: None,
            allowlist_add: None,
            allowlist_remove: None,
        },
        &[],
    )
    .unwrap();

    let config = get_config(&app, pre_propose.clone());
    assert_eq!(
        config,
        Config {
            deposit_info: None,
            submission_policy: PreProposeSubmissionPolicy::Anyone { denylist: vec![] },
        }
    );

    // Error if try to change Specific fields when set to Anyone.
    let err: PreProposeError = app
        .execute_contract(
            core_addr.clone(),
            pre_propose.clone(),
            &ExecuteMsg::UpdateSubmissionPolicy {
                denylist_add: None,
                denylist_remove: None,
                set_dao_members: Some(true),
                allowlist_add: None,
                allowlist_remove: None,
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(
        err,
        PreProposeError::SubmissionPolicy(
            PreProposeSubmissionPolicyError::AnyoneInvalidUpdateFields {}
        )
    );
    let err: PreProposeError = app
        .execute_contract(
            core_addr.clone(),
            pre_propose.clone(),
            &ExecuteMsg::UpdateSubmissionPolicy {
                denylist_add: None,
                denylist_remove: None,
                set_dao_members: None,
                allowlist_add: Some(vec!["ekez".to_string()]),
                allowlist_remove: None,
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(
        err,
        PreProposeError::SubmissionPolicy(
            PreProposeSubmissionPolicyError::AnyoneInvalidUpdateFields {}
        )
    );
    let err: PreProposeError = app
        .execute_contract(
            core_addr.clone(),
            pre_propose.clone(),
            &ExecuteMsg::UpdateSubmissionPolicy {
                denylist_add: None,
                denylist_remove: None,
                set_dao_members: None,
                allowlist_add: None,
                allowlist_remove: Some(vec!["ekez".to_string()]),
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(
        err,
        PreProposeError::SubmissionPolicy(
            PreProposeSubmissionPolicyError::AnyoneInvalidUpdateFields {}
        )
    );

    // Change to Specific policy.
    app.execute_contract(
        core_addr.clone(),
        pre_propose.clone(),
        &ExecuteMsg::UpdateConfig {
            deposit_info: None,
            submission_policy: Some(PreProposeSubmissionPolicy::Specific {
                dao_members: true,
                allowlist: vec![],
                denylist: vec![],
            }),
        },
        &[],
    )
    .unwrap();

    let config = get_config(&app, pre_propose.clone());
    assert_eq!(
        config,
        Config {
            deposit_info: None,
            submission_policy: PreProposeSubmissionPolicy::Specific {
                dao_members: true,
                allowlist: vec![],
                denylist: vec![],
            },
        }
    );

    // Append to denylist, with auto de-dupe.
    app.execute_contract(
        core_addr.clone(),
        pre_propose.clone(),
        &ExecuteMsg::UpdateSubmissionPolicy {
            denylist_add: Some(vec!["ekez".to_string(), "ekez".to_string()]),
            denylist_remove: None,
            set_dao_members: None,
            allowlist_add: None,
            allowlist_remove: None,
        },
        &[],
    )
    .unwrap();

    let config = get_config(&app, pre_propose.clone());
    assert_eq!(
        config,
        Config {
            deposit_info: None,
            submission_policy: PreProposeSubmissionPolicy::Specific {
                dao_members: true,
                allowlist: vec![],
                denylist: vec![Addr::unchecked("ekez")],
            },
        }
    );

    // Add and remove to/from denylist.
    app.execute_contract(
        core_addr.clone(),
        pre_propose.clone(),
        &ExecuteMsg::UpdateSubmissionPolicy {
            denylist_add: Some(vec!["someone".to_string(), "else".to_string()]),
            denylist_remove: Some(vec!["ekez".to_string()]),
            set_dao_members: None,
            allowlist_add: None,
            allowlist_remove: None,
        },
        &[],
    )
    .unwrap();

    let config = get_config(&app, pre_propose.clone());
    assert_eq!(
        config,
        Config {
            deposit_info: None,
            submission_policy: PreProposeSubmissionPolicy::Specific {
                dao_members: true,
                allowlist: vec![],
                denylist: vec![Addr::unchecked("else"), Addr::unchecked("someone")],
            },
        }
    );

    // Remove from denylist.
    app.execute_contract(
        core_addr.clone(),
        pre_propose.clone(),
        &ExecuteMsg::UpdateSubmissionPolicy {
            denylist_add: None,
            denylist_remove: Some(vec!["someone".to_string(), "else".to_string()]),
            set_dao_members: None,
            allowlist_add: None,
            allowlist_remove: None,
        },
        &[],
    )
    .unwrap();

    let config = get_config(&app, pre_propose.clone());
    assert_eq!(
        config,
        Config {
            deposit_info: None,
            submission_policy: PreProposeSubmissionPolicy::Specific {
                dao_members: true,
                allowlist: vec![],
                denylist: vec![]
            },
        }
    );

    // Append to allowlist, with auto de-dupe.
    app.execute_contract(
        core_addr.clone(),
        pre_propose.clone(),
        &ExecuteMsg::UpdateSubmissionPolicy {
            denylist_add: None,
            denylist_remove: None,
            set_dao_members: None,
            allowlist_add: Some(vec!["ekez".to_string(), "ekez".to_string()]),
            allowlist_remove: None,
        },
        &[],
    )
    .unwrap();

    let config = get_config(&app, pre_propose.clone());
    assert_eq!(
        config,
        Config {
            deposit_info: None,
            submission_policy: PreProposeSubmissionPolicy::Specific {
                dao_members: true,
                allowlist: vec![Addr::unchecked("ekez")],
                denylist: vec![],
            },
        }
    );

    // Add and remove to/from allowlist.
    app.execute_contract(
        core_addr.clone(),
        pre_propose.clone(),
        &ExecuteMsg::UpdateSubmissionPolicy {
            denylist_add: None,
            denylist_remove: None,
            set_dao_members: None,
            allowlist_add: Some(vec!["someone".to_string(), "else".to_string()]),
            allowlist_remove: Some(vec!["ekez".to_string()]),
        },
        &[],
    )
    .unwrap();

    let config = get_config(&app, pre_propose.clone());
    assert_eq!(
        config,
        Config {
            deposit_info: None,
            submission_policy: PreProposeSubmissionPolicy::Specific {
                dao_members: true,
                allowlist: vec![Addr::unchecked("else"), Addr::unchecked("someone")],
                denylist: vec![],
            },
        }
    );

    // Remove from allowlist.
    app.execute_contract(
        core_addr.clone(),
        pre_propose.clone(),
        &ExecuteMsg::UpdateSubmissionPolicy {
            denylist_add: None,
            denylist_remove: None,
            set_dao_members: None,
            allowlist_add: None,
            allowlist_remove: Some(vec!["someone".to_string(), "else".to_string()]),
        },
        &[],
    )
    .unwrap();

    let config = get_config(&app, pre_propose.clone());
    assert_eq!(
        config,
        Config {
            deposit_info: None,
            submission_policy: PreProposeSubmissionPolicy::Specific {
                dao_members: true,
                allowlist: vec![],
                denylist: vec![]
            },
        }
    );

    // Setting dao_members to false fails if allowlist is empty.
    let err: PreProposeError = app
        .execute_contract(
            core_addr.clone(),
            pre_propose.clone(),
            &ExecuteMsg::UpdateSubmissionPolicy {
                denylist_add: None,
                denylist_remove: None,
                set_dao_members: Some(false),
                allowlist_add: None,
                allowlist_remove: None,
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(
        err,
        PreProposeError::SubmissionPolicy(PreProposeSubmissionPolicyError::NoOneAllowed {})
    );

    // Set dao_members to false and add allowlist.
    app.execute_contract(
        core_addr.clone(),
        pre_propose.clone(),
        &ExecuteMsg::UpdateSubmissionPolicy {
            denylist_add: None,
            denylist_remove: None,
            set_dao_members: Some(false),
            allowlist_add: Some(vec!["ekez".to_string()]),
            allowlist_remove: None,
        },
        &[],
    )
    .unwrap();

    let config = get_config(&app, pre_propose.clone());
    assert_eq!(
        config,
        Config {
            deposit_info: None,
            submission_policy: PreProposeSubmissionPolicy::Specific {
                dao_members: false,
                allowlist: vec![Addr::unchecked("ekez")],
                denylist: vec![]
            },
        }
    );

    // Errors when allowlist and denylist overlap.
    let err: PreProposeError = app
        .execute_contract(
            core_addr.clone(),
            pre_propose.clone(),
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
    assert_eq!(
        err,
        PreProposeError::SubmissionPolicy(
            PreProposeSubmissionPolicyError::DenylistAllowlistOverlap {}
        )
    );
}

#[test]
fn test_withdraw() {
    let mut app = App::default();

    let DefaultTestSetup {
        core_addr,
        proposal_multiple,
        pre_propose,
    } = setup_default_test(&mut app, None, false);

    let err = withdraw_should_fail(
        &mut app,
        pre_propose.clone(),
        proposal_multiple.as_str(),
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
    let native_pre_propose_id =
        make_pre_proposal(&mut app, pre_propose.clone(), "ekez", &coins(10, "ujuno"));

    // Approver approves
    let native_id = approve_proposal(
        &mut app,
        pre_propose.clone(),
        "approver",
        native_pre_propose_id,
    );

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
    let cw20_pre_propose_id = make_pre_proposal(&mut app, pre_propose.clone(), "ekez", &[]);

    // Approver approves
    let cw20_id = approve_proposal(
        &mut app,
        pre_propose.clone(),
        "approver",
        cw20_pre_propose_id,
    );

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
    vote(&mut app, proposal_multiple.clone(), "ekez", cw20_id, 1);
    execute_proposal(&mut app, proposal_multiple.clone(), "ekez", cw20_id);

    // Make sure the proposal module has fallen back to anyone can
    // propose becuase of our malfunction.
    let proposal_creation_policy: ProposalCreationPolicy = app
        .wrap()
        .query_wasm_smart(
            proposal_multiple.clone(),
            &dao_proposal_multiple::msg::QueryMsg::ProposalCreationPolicy {},
        )
        .unwrap();

    assert_eq!(proposal_creation_policy, ProposalCreationPolicy::Anyone {});

    // Close out the native proposal and it's deposit as well.
    vote(&mut app, proposal_multiple.clone(), "ekez", native_id, 2);
    close_proposal(&mut app, proposal_multiple.clone(), "ekez", native_id);
    withdraw(
        &mut app,
        pre_propose.clone(),
        core_addr.as_str(),
        Some(UncheckedDenom::Native("ujuno".to_string())),
    );
    let balance = get_balance_native(&app, core_addr.as_str(), "ujuno");
    assert_eq!(balance, Uint128::new(30));
}
