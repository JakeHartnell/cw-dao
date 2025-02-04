use anyhow::Result;
use cosm_orc::orchestrator::{Coin, Key, SigningKey};
use cosm_orc::{config::cfg::Config, orchestrator::cosm_orc::CosmOrc};
use cosmwasm_std::{to_json_binary, Decimal, Empty, Uint128};
use cw20::Cw20Coin;
use dao_interface::state::{Admin, ModuleInstantiateInfo};
use dao_voting::pre_propose::PreProposeSubmissionPolicy;
use dao_voting::{
    deposit::{DepositRefundPolicy, DepositToken, UncheckedDepositInfo, VotingModuleTokenType},
    pre_propose::PreProposeInfo,
    threshold::PercentageThreshold,
    threshold::Threshold,
};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::time::Duration;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Account {
    name: String,
    address: String,
    mnemonic: String,
}

fn main() -> Result<()> {
    env_logger::init();

    let config = env::var("CONFIG").expect("missing yaml CONFIG env var");
    let mut cfg = Config::from_yaml(&config)?;
    let mut orc = CosmOrc::new(cfg.clone(), false)?;

    // use first test user as DAO admin, and only DAO member:
    let accounts: Vec<Account> =
        serde_json::from_slice(&fs::read("ci/configs/test_accounts.json")?)?;
    let account = accounts[0].clone();

    let key = SigningKey {
        name: account.name,
        key: Key::Mnemonic(account.mnemonic),
        derivation_path: cfg.chain_cfg.derivation_path.clone(),
    };
    let addr = account.address;

    orc.poll_for_n_blocks(1, Duration::from_millis(20_000), true)?;

    orc.store_contracts("artifacts", &key, None)?;

    let msg = dao_interface::msg::InstantiateMsg {
        admin: Some(addr.clone()),
        name: "DAO DAO".to_string(),
        description: "A DAO that makes DAO tooling".to_string(),
        image_url: Some("https://zmedley.com/raw_logo.png".to_string()),
        dao_uri: None,
        automatically_add_cw20s: false,
        automatically_add_cw721s: false,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: orc.contract_map.code_id("dao_voting_cw20_staked")?,
            msg: to_json_binary(&dao_voting_cw20_staked::msg::InstantiateMsg {
                token_info: dao_voting_cw20_staked::msg::TokenInfo::New {
                    code_id: orc.contract_map.code_id("cw20_base")?,
                    label: "DAO DAO Gov token".to_string(),
                    name: "DAO".to_string(),
                    symbol: "DAO".to_string(),
                    decimals: 6,
                    initial_balances: vec![Cw20Coin {
                        address: addr.clone(),
                        amount: Uint128::new(100_000_000),
                    }],
                    marketing: None,
                    staking_code_id: orc.contract_map.code_id("cw20_stake")?,
                    unstaking_duration: Some(cw_utils::Duration::Time(1209600)),
                    initial_dao_balance: None,
                },
                active_threshold: None,
            })?,
            funds: vec![],
            admin: Some(Admin::CoreModule {}),
            label: "DAO DAO Voting Module".to_string(),
        },
        proposal_modules_instantiate_info: vec![ModuleInstantiateInfo {
            code_id: orc.contract_map.code_id("dao_proposal_single")?,
            msg: to_json_binary(&dao_proposal_single::msg::InstantiateMsg {
                min_voting_period: None,
                threshold: Threshold::ThresholdQuorum {
                    threshold: PercentageThreshold::Majority {},
                    quorum: PercentageThreshold::Percent(Decimal::percent(10)),
                },
                max_voting_period: cw_utils::Duration::Time(432000),
                allow_revoting: false,
                only_members_execute: true,
                pre_propose_info: PreProposeInfo::ModuleMayPropose {
                    info: ModuleInstantiateInfo {
                        code_id: orc.contract_map.code_id("dao_pre_propose_single")?,
                        msg: to_json_binary(&dao_pre_propose_single::InstantiateMsg {
                            deposit_info: Some(UncheckedDepositInfo {
                                denom: DepositToken::VotingModuleToken {
                                    token_type: VotingModuleTokenType::Cw20,
                                },
                                amount: Uint128::new(1000000000),
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
                close_proposal_on_execution_failure: false,
                veto: None,
                delegation_module: None,
            })?,
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "DAO DAO Proposal Module".to_string(),
        }],
        initial_items: None,
    };

    // Init dao dao dao with an initial treasury of 9000000 tokens
    orc.instantiate(
        "dao_dao_core",
        "dao_init",
        &msg,
        &key,
        Some(addr.parse()?),
        vec![Coin {
            denom: cfg.chain_cfg.denom.parse()?,
            amount: 9000000,
        }],
    )?;

    orc.instantiate(
        "cw_admin_factory",
        "admin_factory_init",
        &cw_admin_factory::msg::InstantiateMsg { admin: None },
        &key,
        None,
        vec![],
    )?;

    println!(" ------------------------ ");
    println!("Config Variables\n");

    println!("Admin user address: {addr}");

    println!(
        "NEXT_PUBLIC_CW20_CODE_ID={}",
        orc.contract_map.code_id("cw20_base")?
    );
    println!(
        "NEXT_PUBLIC_CW4GROUP_CODE_ID={}",
        orc.contract_map.code_id("cw4_group")?
    );
    println!(
        "NEXT_PUBLIC_CWCORE_CODE_ID={}",
        orc.contract_map.code_id("dao_dao_core")?
    );
    println!(
        "NEXT_PUBLIC_CWPROPOSALSINGLE_CODE_ID={}",
        orc.contract_map.code_id("dao_proposal_single")?
    );
    println!(
        "NEXT_PUBLIC_CW4VOTING_CODE_ID={}",
        orc.contract_map.code_id("dao_voting_cw4")?
    );
    println!(
        "NEXT_PUBLIC_CW20STAKEDBALANCEVOTING_CODE_ID={}",
        orc.contract_map.code_id("dao_voting_cw20_staked")?
    );
    println!(
        "NEXT_PUBLIC_STAKECW20_CODE_ID={}",
        orc.contract_map.code_id("cw20_stake")?
    );
    println!(
        "NEXT_PUBLIC_DAO_CONTRACT_ADDRESS={}",
        orc.contract_map.address("dao_dao_core")?
    );
    println!(
        "NEXT_PUBLIC_V1_FACTORY_CONTRACT_ADDRESS={}",
        orc.contract_map.address("cw_admin_factory")?
    );

    // Persist contract code_ids in local.yaml so we can use SKIP_CONTRACT_STORE locally to avoid having to re-store them again
    cfg.contract_deploy_info
        .clone_from(orc.contract_map.deploy_info());
    fs::write(
        "ci/configs/cosm-orc/local.yaml",
        serde_yaml::to_string(&cfg)?,
    )?;

    Ok(())
}
