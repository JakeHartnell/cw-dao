use super::chain::Chain;
use anyhow::Result;
use cosm_orc::orchestrator::SigningKey;
use cosmwasm_std::{to_json_binary, CosmosMsg, Decimal, Empty, Uint128};
use cw20::Cw20Coin;
use cw_utils::Duration;
use dao_interface::query::DumpStateResponse;
use dao_interface::state::{Admin, ModuleInstantiateInfo};
use dao_voting::{
    deposit::{DepositRefundPolicy, DepositToken, UncheckedDepositInfo, VotingModuleTokenType},
    pre_propose::{PreProposeInfo, PreProposeSubmissionPolicy, ProposalCreationPolicy},
    threshold::PercentageThreshold,
    threshold::Threshold,
    voting::Vote,
};

pub const DEPOSIT_AMOUNT: Uint128 = Uint128::new(1_000_000);

#[derive(Debug)]
pub struct DaoState {
    pub addr: String,
    pub state: DumpStateResponse,
}

pub fn create_dao(
    chain: &mut Chain,
    admin: Option<String>,
    op_name: &str,
    user_addr: String,
    key: &SigningKey,
) -> Result<DaoState> {
    let msg = dao_interface::msg::InstantiateMsg {
        dao_uri: None,
        admin,
        name: "DAO DAO".to_string(),
        description: "A DAO that makes DAO tooling".to_string(),
        image_url: None,
        automatically_add_cw20s: false,
        automatically_add_cw721s: false,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: chain.orc.contract_map.code_id("dao_voting_cw20_staked")?,
            msg: to_json_binary(&dao_voting_cw20_staked::msg::InstantiateMsg {
                token_info: dao_voting_cw20_staked::msg::TokenInfo::New {
                    code_id: chain.orc.contract_map.code_id("cw20_base")?,
                    label: "DAO DAO Gov token".to_string(),
                    name: "DAO".to_string(),
                    symbol: "DAO".to_string(),
                    decimals: 6,
                    initial_balances: vec![Cw20Coin {
                        address: user_addr,
                        amount: Uint128::new(100_000_000),
                    }],
                    marketing: None,
                    staking_code_id: chain.orc.contract_map.code_id("cw20_stake")?,
                    unstaking_duration: Some(Duration::Time(1209600)),
                    initial_dao_balance: None,
                },
                active_threshold: None,
            })?,
            admin: Some(Admin::CoreModule {}),
            label: "DAO DAO Voting Module".to_string(),
            funds: vec![],
        },
        proposal_modules_instantiate_info: vec![ModuleInstantiateInfo {
            code_id: chain.orc.contract_map.code_id("dao_proposal_single")?,
            msg: to_json_binary(&dao_proposal_single::msg::InstantiateMsg {
                min_voting_period: None,
                threshold: Threshold::ThresholdQuorum {
                    threshold: PercentageThreshold::Majority {},
                    quorum: PercentageThreshold::Percent(Decimal::percent(35)),
                },
                max_voting_period: Duration::Time(432000),
                allow_revoting: false,
                only_members_execute: true,
                close_proposal_on_execution_failure: false,
                pre_propose_info: PreProposeInfo::ModuleMayPropose {
                    info: ModuleInstantiateInfo {
                        code_id: chain.orc.contract_map.code_id("dao_pre_propose_single")?,
                        msg: to_json_binary(&dao_pre_propose_single::InstantiateMsg {
                            deposit_info: Some(UncheckedDepositInfo {
                                denom: DepositToken::VotingModuleToken {
                                    token_type: VotingModuleTokenType::Cw20,
                                },
                                amount: DEPOSIT_AMOUNT,
                                refund_policy: DepositRefundPolicy::OnlyPassed,
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
                        label: "DAO DAO Pre-Propose Module".to_string(),
                    },
                },
                veto: None,
                delegation_module: None,
            })?,
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "DAO DAO Proposal Module".to_string(),
        }],
        initial_items: None,
    };

    chain
        .orc
        .instantiate("dao_dao_core", op_name, &msg, key, None, vec![])?;

    // add proposal, pre-propose, voting, cw20_stake, and cw20_base
    // contracts to the orc contract map.

    let state: DumpStateResponse = chain
        .orc
        .query("dao_dao_core", &dao_interface::msg::QueryMsg::DumpState {})?
        .data()
        .unwrap();
    chain
        .orc
        .contract_map
        .add_address(
            "dao_proposal_single",
            state.proposal_modules[0].address.to_string(),
        )
        .unwrap();

    let ProposalCreationPolicy::Module { addr: pre_propose } = chain
        .orc
        .query(
            "dao_proposal_single",
            &dao_proposal_single::msg::QueryMsg::ProposalCreationPolicy {},
        )
        .unwrap()
        .data()
        .unwrap()
    else {
        panic!("expected pre-propose module")
    };
    chain
        .orc
        .contract_map
        .add_address("dao_pre_propose_single", pre_propose)
        .unwrap();

    chain
        .orc
        .contract_map
        .add_address("dao_voting_cw20_staked", state.voting_module.to_string())
        .unwrap();
    let cw20_stake: String = chain
        .orc
        .query(
            "dao_voting_cw20_staked",
            &dao_voting_cw20_staked::msg::QueryMsg::StakingContract {},
        )
        .unwrap()
        .data()
        .unwrap();
    chain
        .orc
        .contract_map
        .add_address("cw20_stake", cw20_stake)
        .unwrap();
    let cw20_base: String = chain
        .orc
        .query(
            "dao_voting_cw20_staked",
            &dao_voting_cw20_staked::msg::QueryMsg::TokenContract {},
        )
        .unwrap()
        .data()
        .unwrap();
    chain
        .orc
        .contract_map
        .add_address("cw20_base", cw20_base)
        .unwrap();

    Ok(DaoState {
        addr: chain.orc.contract_map.address("dao_dao_core")?,
        state,
    })
}

pub fn stake_tokens(chain: &mut Chain, how_many: u128, key: &SigningKey) {
    chain
        .orc
        .execute(
            "cw20_base",
            "send_and_stake_cw20",
            &cw20::Cw20ExecuteMsg::Send {
                contract: chain.orc.contract_map.address("cw20_stake").unwrap(),
                amount: Uint128::new(how_many),
                msg: to_json_binary(&cw20_stake::msg::ReceiveMsg::Stake {}).unwrap(),
            },
            key,
            vec![],
        )
        .unwrap();
    chain
        .orc
        .poll_for_n_blocks(1, std::time::Duration::from_millis(20_000), false)
        .unwrap();
}

pub fn create_proposal(
    chain: &mut Chain,
    msgs: Vec<CosmosMsg>,
    key: &SigningKey,
) -> Result<dao_proposal_single::query::ProposalResponse> {
    let next_id: u64 = chain
        .orc
        .query(
            "dao_proposal_single",
            &dao_proposal_single::msg::QueryMsg::NextProposalId {},
        )
        .unwrap()
        .data()
        .unwrap();

    // increase allowance to pay proposal deposit.
    chain
        .orc
        .execute(
            "cw20_base",
            "cw20_base_increase_allowance",
            &cw20::Cw20ExecuteMsg::IncreaseAllowance {
                spender: chain
                    .orc
                    .contract_map
                    .address("dao_pre_propose_single")
                    .unwrap(),
                amount: DEPOSIT_AMOUNT,
                expires: None,
            },
            key,
            vec![],
        )
        .unwrap();

    chain
        .orc
        .execute(
            "dao_pre_propose_single",
            "pre_propose_propose",
            &dao_pre_propose_single::ExecuteMsg::Propose {
                msg: dao_pre_propose_single::ProposeMessage::Propose {
                    title: "title".to_string(),
                    description: "desc".to_string(),
                    msgs,
                    vote: None,
                },
            },
            key,
            vec![],
        )
        .unwrap();

    let r = chain
        .orc
        .query(
            "dao_proposal_single",
            &dao_proposal_single::msg::QueryMsg::Proposal {
                proposal_id: next_id,
            },
        )
        .unwrap()
        .data()
        .unwrap();

    Ok(r)
}

pub fn vote(chain: &mut Chain, proposal_id: u64, vote: Vote, key: &SigningKey) {
    chain
        .orc
        .execute(
            "dao_proposal_single",
            "dao_proposal_single_vote",
            &dao_proposal_single::msg::ExecuteMsg::Vote {
                proposal_id,
                vote,
                rationale: None,
            },
            key,
            vec![],
        )
        .unwrap();
}

pub fn execute(chain: &mut Chain, proposal_id: u64, key: &SigningKey) {
    chain
        .orc
        .execute(
            "dao_proposal_single",
            "dao_proposal_single_vote",
            &dao_proposal_single::msg::ExecuteMsg::Execute { proposal_id },
            key,
            vec![],
        )
        .unwrap();
}
