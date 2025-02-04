// The code is used in tests but reported as dead code
// see https://github.com/rust-lang/rust/issues/46379
#![allow(dead_code)]

use crate::{
    msg::{ExecuteMsg, InstantiateMsg, NftContract, QueryMsg},
    state::Config,
};

use cosmwasm_std::{to_json_binary, Addr, Coin, Decimal, Empty, WasmMsg};
use cw_utils::Duration;
use dao_interface::{
    msg::QueryMsg as DaoQueryMsg,
    state::{Admin, ModuleInstantiateInfo, ProposalModule},
};
use dao_voting::{
    pre_propose::PreProposeInfo, threshold::PercentageThreshold, threshold::Threshold,
};

use cw721_base::msg::{ExecuteMsg as Cw721ExecuteMsg, InstantiateMsg as Cw721InstantiateMsg};
use dao_testing::test_tube::{
    cw721_base::Cw721Base, dao_dao_core::DaoCore, dao_proposal_single::DaoProposalSingle,
    dao_test_custom_factory::CustomFactoryContract,
};
use dao_voting::threshold::ActiveThreshold;
use osmosis_test_tube::{
    osmosis_std::types::cosmwasm::wasm::v1::MsgExecuteContractResponse, Account, Bank, Module,
    OsmosisTestApp, RunnerError, RunnerExecuteResult, RunnerResult, SigningAccount, Wasm,
};
use serde::de::DeserializeOwned;
use std::path::PathBuf;

pub const DENOM: &str = "ucat";
pub const JUNO: &str = "ujuno";

pub struct TestEnv<'a> {
    pub app: &'a OsmosisTestApp,
    pub dao: DaoCore<'a>,
    pub proposal_single: DaoProposalSingle<'a>,
    pub custom_factory: CustomFactoryContract<'a>,
    pub vp_contract: Cw721VotingContract<'a>,
    pub accounts: Vec<SigningAccount>,
    pub cw721: Cw721Base<'a>,
}

impl<'a> TestEnv<'a> {
    pub fn bank(&self) -> Bank<'_, OsmosisTestApp> {
        Bank::new(self.app)
    }
}

pub struct TestEnvBuilder {}

impl TestEnvBuilder {
    pub fn new() -> Self {
        Self {}
    }

    // Full DAO setup
    pub fn setup(self, app: &'_ OsmosisTestApp) -> TestEnv<'_> {
        let accounts = app
            .init_accounts(&[Coin::new(1000000000000000u128, "uosmo")], 10)
            .unwrap();
        // Upload all needed code ids
        let vp_contract_id = Cw721VotingContract::upload(app, &accounts[0]).unwrap();
        let proposal_single_id = DaoProposalSingle::upload(app, &accounts[0]).unwrap();
        let cw721_id = Cw721Base::upload(app, &accounts[0]).unwrap();

        // Instantiate Custom Factory
        let custom_factory = CustomFactoryContract::new(
            app,
            &dao_test_custom_factory::msg::InstantiateMsg {},
            &accounts[0],
        )
        .unwrap();

        let msg = dao_interface::msg::InstantiateMsg {
            dao_uri: None,
            admin: None,
            name: "DAO DAO".to_string(),
            description: "A DAO that makes DAO tooling".to_string(),
            image_url: None,
            automatically_add_cw20s: false,
            automatically_add_cw721s: false,
            voting_module_instantiate_info: ModuleInstantiateInfo {
                code_id: vp_contract_id,
                msg: to_json_binary(&InstantiateMsg {
                    nft_contract: NftContract::Factory(
                        to_json_binary(&WasmMsg::Execute {
                            contract_addr: custom_factory.contract_addr.clone(),
                            msg: to_json_binary(
                                &dao_test_custom_factory::msg::ExecuteMsg::NftFactory {
                                    code_id: cw721_id,
                                    cw721_instantiate_msg: Cw721InstantiateMsg {
                                        name: "Test NFT".to_string(),
                                        symbol: "TEST".to_string(),
                                        minter: accounts[0].address(),
                                    },
                                    initial_nfts: vec![to_json_binary(&Cw721ExecuteMsg::<
                                        Empty,
                                        Empty,
                                    >::Mint {
                                        owner: accounts[0].address(),
                                        token_uri: Some("https://example.com".to_string()),
                                        token_id: "1".to_string(),
                                        extension: Empty {},
                                    })
                                    .unwrap()],
                                },
                            )
                            .unwrap(),
                            funds: vec![],
                        })
                        .unwrap(),
                    ),
                    unstaking_duration: None,
                    active_threshold: Some(ActiveThreshold::Percentage {
                        percent: Decimal::percent(1),
                    }),
                })
                .unwrap(),
                admin: Some(Admin::CoreModule {}),
                funds: vec![],
                label: "DAO DAO Voting Module".to_string(),
            },
            proposal_modules_instantiate_info: vec![ModuleInstantiateInfo {
                code_id: proposal_single_id,
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
                    pre_propose_info: PreProposeInfo::AnyoneMayPropose {},
                    veto: None,
                    delegation_module: None,
                })
                .unwrap(),
                admin: Some(Admin::CoreModule {}),
                funds: vec![],
                label: "DAO DAO Proposal Module".to_string(),
            }],
            initial_items: None,
        };

        // Instantiate DAO
        let dao = DaoCore::new(app, &msg, &accounts[0], &[]).unwrap();

        // Get voting module address, setup vp_contract helper
        let vp_addr: Addr = dao.query(&DaoQueryMsg::VotingModule {}).unwrap();
        let vp_contract =
            Cw721VotingContract::new_with_values(app, vp_contract_id, vp_addr.to_string()).unwrap();

        let vp_config: Config = vp_contract.query(&QueryMsg::Config {}).unwrap();

        let cw721 =
            Cw721Base::new_with_values(app, cw721_id, vp_config.nft_address.to_string()).unwrap();

        // Get proposal module address, setup proposal_single helper
        let proposal_modules: Vec<ProposalModule> = dao
            .query(&DaoQueryMsg::ProposalModules {
                limit: None,
                start_after: None,
            })
            .unwrap();
        let proposal_single = DaoProposalSingle::new_with_values(
            app,
            proposal_single_id,
            proposal_modules[0].address.to_string(),
        )
        .unwrap();

        TestEnv {
            app,
            dao,
            vp_contract,
            proposal_single,
            custom_factory,
            accounts,
            cw721,
        }
    }
}

#[derive(Debug)]
pub struct Cw721VotingContract<'a> {
    pub app: &'a OsmosisTestApp,
    pub contract_addr: String,
    pub code_id: u64,
}

impl<'a> Cw721VotingContract<'a> {
    pub fn deploy(
        app: &'a OsmosisTestApp,
        instantiate_msg: &InstantiateMsg,
        signer: &SigningAccount,
    ) -> Result<Self, RunnerError> {
        let wasm = Wasm::new(app);

        let code_id = wasm
            .store_code(&Self::get_wasm_byte_code(), None, signer)?
            .data
            .code_id;

        let contract_addr = wasm
            .instantiate(
                code_id,
                &instantiate_msg,
                Some(&signer.address()),
                None,
                &[],
                signer,
            )?
            .data
            .address;

        Ok(Self {
            app,
            code_id,
            contract_addr,
        })
    }

    pub fn new_with_values(
        app: &'a OsmosisTestApp,
        code_id: u64,
        contract_addr: String,
    ) -> Result<Self, RunnerError> {
        Ok(Self {
            app,
            code_id,
            contract_addr,
        })
    }

    /// uploads contract and returns a code ID
    pub fn upload(app: &OsmosisTestApp, signer: &SigningAccount) -> Result<u64, RunnerError> {
        let wasm = Wasm::new(app);

        let code_id = wasm
            .store_code(&Self::get_wasm_byte_code(), None, signer)?
            .data
            .code_id;

        Ok(code_id)
    }

    pub fn instantiate(
        app: &'a OsmosisTestApp,
        code_id: u64,
        instantiate_msg: &InstantiateMsg,
        signer: &SigningAccount,
    ) -> Result<Self, RunnerError> {
        let wasm = Wasm::new(app);
        let contract_addr = wasm
            .instantiate(
                code_id,
                &instantiate_msg,
                Some(&signer.address()),
                None,
                &[],
                signer,
            )?
            .data
            .address;

        Ok(Self {
            app,
            code_id,
            contract_addr,
        })
    }

    pub fn execute(
        &self,
        msg: &ExecuteMsg,
        funds: &[Coin],
        signer: &SigningAccount,
    ) -> RunnerExecuteResult<MsgExecuteContractResponse> {
        let wasm = Wasm::new(self.app);
        wasm.execute(&self.contract_addr, msg, funds, signer)
    }

    pub fn query<T>(&self, msg: &QueryMsg) -> RunnerResult<T>
    where
        T: ?Sized + DeserializeOwned,
    {
        let wasm = Wasm::new(self.app);
        wasm.query(&self.contract_addr, msg)
    }

    fn get_wasm_byte_code() -> Vec<u8> {
        let manifest_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let byte_code = std::fs::read(
            manifest_path
                .join("..")
                .join("..")
                .join("..")
                .join("artifacts")
                .join("dao_voting_cw721_staked.wasm"),
        );
        match byte_code {
            Ok(byte_code) => byte_code,
            // On arm processors, the above path is not found, so we try the following path
            Err(_) => std::fs::read(
                manifest_path
                    .join("..")
                    .join("..")
                    .join("..")
                    .join("artifacts")
                    .join("dao_voting_cw721_staked-aarch64.wasm"),
            )
            .unwrap(),
        }
    }
}
