use std::{
    fmt::{Debug, Display},
    ops::{Deref, DerefMut},
};

use cosmwasm_std::{
    to_json_binary, Addr, Coin, CosmosMsg, Empty, QuerierWrapper, Timestamp, Uint128,
};
use cw20::Cw20Coin;
use cw_multi_test::{error::AnyResult, App, AppResponse, Contract, Executor};
use cw_utils::Duration;
use serde::Serialize;

use super::*;
use crate::contracts::*;

#[derive(Clone, Debug)]
pub struct TestDao<Extra = Empty> {
    pub core_addr: Addr,
    pub voting_module_addr: Addr,
    /// proposal modules in the form (pre-propose module, proposal module). if
    /// the pre-propose module is None, then it does not exist.
    pub proposal_modules: Vec<(Option<Addr>, Addr)>,
    pub x: Extra,
}

pub struct DaoTestingSuiteBase {
    pub app: App,

    // Code IDs
    // DAO stuff
    pub core_id: u64,
    pub admin_factory_id: u64,
    pub proposal_single_id: u64,
    pub proposal_multiple_id: u64,
    pub proposal_sudo_id: u64,
    pub pre_propose_approval_single_id: u64,
    pub pre_propose_single_id: u64,
    pub pre_propose_multiple_id: u64,
    pub pre_propose_approver_id: u64,
    pub voting_cw4_id: u64,
    pub voting_cw20_staked_id: u64,
    pub voting_cw20_balance_id: u64,
    pub voting_cw721_staked_id: u64,
    pub voting_token_staked_id: u64,
    pub cw20_stake_id: u64,
    pub rewards_distributor_id: u64,
    // External stuff
    pub cw4_group_id: u64,
    pub cw20_base_id: u64,
    pub cw721_base_id: u64,

    // Addresses
    pub admin_factory_addr: Addr,
}

pub trait DaoTestingSuite<Extra = Empty>: Deref + DerefMut {
    /// get the testing suite base
    fn base(&self) -> &DaoTestingSuiteBase;

    /// get the mutable testing suite base
    fn base_mut(&mut self) -> &mut DaoTestingSuiteBase;

    /// get the voting module info to instantiate the DAO with
    fn get_voting_module_info(&self) -> dao_interface::state::ModuleInstantiateInfo;

    /// get the extra DAO fields
    fn get_dao_extra(&self, _dao: &TestDao) -> Extra;

    /// perform additional setup for the DAO after it is created. empty default
    /// implementation makes this optional.
    fn dao_setup(&mut self, _dao: &mut TestDao<Extra>) {}

    /// build the DAO. no need to override this.
    fn dao(&mut self) -> TestDao<Extra> {
        let voting_module_info = self.get_voting_module_info();

        let proposal_module_infos =
            vec![dao_interface::state::ModuleInstantiateInfo {
            code_id: self.base().proposal_single_id,
            msg: to_json_binary(&dao_proposal_single::msg::InstantiateMsg {
                threshold: dao_voting::threshold::Threshold::AbsolutePercentage {
                    percentage: dao_voting::threshold::PercentageThreshold::Majority {},
                },
                max_voting_period: Duration::Height(10),
                min_voting_period: None,
                only_members_execute: true,
                allow_revoting: false,
                pre_propose_info: dao_voting::pre_propose::PreProposeInfo::ModuleMayPropose {
                    info: dao_interface::state::ModuleInstantiateInfo {
                        code_id: self.base().pre_propose_single_id,
                        msg: to_json_binary(&dao_pre_propose_single::InstantiateMsg {
                            deposit_info: None,
                            submission_policy:
                                dao_voting::pre_propose::PreProposeSubmissionPolicy::Specific {
                                    dao_members: true,
                                    allowlist: vec![],
                                    denylist: vec![],
                                },
                            extension: Empty {},
                        })
                        .unwrap(),
                        admin: Some(dao_interface::state::Admin::CoreModule {}),
                        funds: vec![],
                        label: "single choice pre-propose module".to_string(),
                    },
                },
                close_proposal_on_execution_failure: true,
                veto: None,
                delegation_module: None,
            })
            .unwrap(),
            admin: Some(dao_interface::state::Admin::CoreModule {}),
            funds: vec![],
            label: "single choice proposal module".to_string(),
        },
        dao_interface::state::ModuleInstantiateInfo {
            code_id: self.base().proposal_multiple_id,
            msg: to_json_binary(&dao_proposal_multiple::msg::InstantiateMsg {
                voting_strategy: dao_voting::multiple_choice::VotingStrategy::SingleChoice {
                    quorum: dao_voting::threshold::PercentageThreshold::Majority {},
                },
                max_voting_period: Duration::Height(10),
                min_voting_period: None,
                only_members_execute: true,
                allow_revoting: false,
                pre_propose_info: dao_voting::pre_propose::PreProposeInfo::ModuleMayPropose {
                    info: dao_interface::state::ModuleInstantiateInfo {
                        code_id: self.base().pre_propose_multiple_id,
                        msg: to_json_binary(&dao_pre_propose_multiple::InstantiateMsg {
                            deposit_info: None,
                            submission_policy:
                                dao_voting::pre_propose::PreProposeSubmissionPolicy::Specific {
                                    dao_members: true,
                                    allowlist: vec![],
                                    denylist: vec![],
                                },
                            extension: Empty {},
                        })
                        .unwrap(),
                        admin: Some(dao_interface::state::Admin::CoreModule {}),
                        funds: vec![],
                        label: "multiple choice pre-propose module".to_string(),
                    },
                },
                close_proposal_on_execution_failure: true,
                veto: None,
                delegation_module: None,
            })
            .unwrap(),
            admin: Some(dao_interface::state::Admin::CoreModule {}),
            funds: vec![],
            label: "multiple choice proposal module".to_string(),
        }];

        // create the DAO using the base testing suite
        let dao = self
            .base_mut()
            .build(voting_module_info, proposal_module_infos);

        // perform additional queries to get extra fields for DAO struct
        let x = self.get_dao_extra(&dao);

        let mut dao = TestDao {
            core_addr: dao.core_addr,
            voting_module_addr: dao.voting_module_addr,
            proposal_modules: dao.proposal_modules,
            x,
        };

        // perform additional setup after the DAO is created
        self.dao_setup(&mut dao);

        dao
    }
}

// CONSTRUCTOR
impl DaoTestingSuiteBase {
    pub fn base() -> Self {
        let mut app = App::default();

        // start at 0 height and time
        app.update_block(|b| {
            b.height = 0;
            b.time = Timestamp::from_seconds(0);
        });

        let core_id = app.store_code(dao_dao_core_contract());
        let admin_factory_id = app.store_code(cw_admin_factory_contract());
        let proposal_single_id = app.store_code(dao_proposal_single_contract());
        let proposal_multiple_id = app.store_code(dao_proposal_multiple_contract());
        let proposal_sudo_id = app.store_code(dao_proposal_sudo_contract());
        let pre_propose_approval_single_id =
            app.store_code(dao_pre_propose_approval_single_contract());
        let pre_propose_single_id = app.store_code(dao_pre_propose_single_contract());
        let pre_propose_multiple_id = app.store_code(dao_pre_propose_multiple_contract());
        let pre_propose_approver_id = app.store_code(dao_pre_propose_approver_contract());
        let voting_cw4_id = app.store_code(dao_voting_cw4_contract());
        let voting_cw20_staked_id = app.store_code(dao_voting_cw20_staked_contract());
        let voting_cw20_balance_id = app.store_code(dao_voting_cw20_balance_contract());
        let voting_cw721_staked_id = app.store_code(dao_voting_cw721_staked_contract());
        let voting_token_staked_id = app.store_code(dao_voting_token_staked_contract());
        let cw20_stake_id = app.store_code(cw20_stake_contract());
        let rewards_distributor_id = app.store_code(dao_rewards_distributor_contract());

        let cw4_group_id = app.store_code(cw4_group_contract());
        let cw20_base_id = app.store_code(cw20_base_contract());
        let cw721_base_id = app.store_code(cw721_base_contract());

        let admin_factory_addr = app
            .instantiate_contract(
                admin_factory_id,
                Addr::unchecked(OWNER),
                &cw_admin_factory::msg::InstantiateMsg { admin: None },
                &[],
                "admin factory",
                None,
            )
            .unwrap();

        Self {
            app,

            core_id,
            admin_factory_id,
            proposal_single_id,
            proposal_multiple_id,
            proposal_sudo_id,
            pre_propose_approval_single_id,
            pre_propose_single_id,
            pre_propose_multiple_id,
            pre_propose_approver_id,
            voting_cw4_id,
            voting_cw20_staked_id,
            voting_cw20_balance_id,
            voting_cw721_staked_id,
            voting_token_staked_id,
            cw20_stake_id,
            rewards_distributor_id,
            cw4_group_id,
            cw20_base_id,
            cw721_base_id,

            admin_factory_addr,
        }
    }

    pub fn cw4(&mut self) -> DaoTestingSuiteCw4 {
        DaoTestingSuiteCw4::new(self)
    }

    pub fn cw20(&mut self) -> DaoTestingSuiteCw20 {
        DaoTestingSuiteCw20::new(self)
    }

    pub fn cw721(&mut self) -> DaoTestingSuiteCw721 {
        DaoTestingSuiteCw721::new(self)
    }

    pub fn token(&mut self) -> DaoTestingSuiteToken {
        DaoTestingSuiteToken::new(self)
    }
}

// DAO CREATION
impl DaoTestingSuiteBase {
    pub fn build(
        &mut self,
        voting_module_instantiate_info: dao_interface::state::ModuleInstantiateInfo,
        proposal_modules_instantiate_info: Vec<dao_interface::state::ModuleInstantiateInfo>,
    ) -> TestDao<Empty> {
        let init = dao_interface::msg::InstantiateMsg {
            admin: None,
            name: "DAO DAO".to_string(),
            description: "A DAO that builds DAOs.".to_string(),
            image_url: None,
            automatically_add_cw20s: false,
            automatically_add_cw721s: false,
            voting_module_instantiate_info,
            proposal_modules_instantiate_info,
            initial_items: None,
            dao_uri: None,
        };

        let res = self
            .app
            .execute_contract(
                Addr::unchecked(OWNER),
                self.admin_factory_addr.clone(),
                &cw_admin_factory::msg::ExecuteMsg::InstantiateContractWithSelfAdmin {
                    instantiate_msg: to_json_binary(&init).unwrap(),
                    code_id: self.core_id,
                    label: "DAO DAO".to_string(),
                },
                &[],
            )
            .unwrap();

        // get core address from the instantiate event
        let instantiate_event = &res.events[2];
        assert_eq!(instantiate_event.ty, "instantiate");
        let core = Addr::unchecked(instantiate_event.attributes[0].value.clone());

        // get voting module address
        let voting_module_addr: Addr = self
            .app
            .wrap()
            .query_wasm_smart(&core, &dao_interface::msg::QueryMsg::VotingModule {})
            .unwrap();

        // get proposal modules
        let proposal_modules: Vec<dao_interface::state::ProposalModule> = self
            .app
            .wrap()
            .query_wasm_smart(
                &core,
                &dao_interface::msg::QueryMsg::ProposalModules {
                    start_after: None,
                    limit: None,
                },
            )
            .unwrap();

        let proposal_modules = proposal_modules
            .into_iter()
            .map(|p| -> (Option<Addr>, Addr) {
                let pre_propose_module: dao_voting::pre_propose::ProposalCreationPolicy = self
                    .app
                    .wrap()
                    .query_wasm_smart(
                        &p.address,
                        &dao_proposal_single::msg::QueryMsg::ProposalCreationPolicy {},
                    )
                    .unwrap();

                match pre_propose_module {
                    dao_voting::pre_propose::ProposalCreationPolicy::Anyone {} => (None, p.address),
                    dao_voting::pre_propose::ProposalCreationPolicy::Module { addr } => {
                        (Some(addr), p.address)
                    }
                }
            })
            .collect::<Vec<_>>();

        TestDao {
            core_addr: core,
            voting_module_addr,
            proposal_modules,
            x: Empty::default(),
        }
    }
}

// UTILITIES
impl DaoTestingSuiteBase {
    /// advance the block height by N
    pub fn advance_blocks(&mut self, n: u64) {
        self.app.update_block(|b| b.height += n);
    }

    /// advance the block height by one
    pub fn advance_block(&mut self) {
        self.advance_blocks(1);
    }

    /// store a contract given its maker function and return its code ID
    pub fn store(&mut self, contract_maker: impl FnOnce() -> Box<dyn Contract<Empty>>) -> u64 {
        self.app.store_code(contract_maker())
    }

    /// instantiate a smart contract and return its address
    pub fn instantiate<T: Serialize + Debug>(
        &mut self,
        code_id: u64,
        sender: impl Into<String>,
        init_msg: &T,
        send_funds: &[Coin],
        label: impl Into<String>,
        admin: Option<String>,
    ) -> Addr {
        self.app
            .instantiate_contract(
                code_id,
                Addr::unchecked(sender),
                init_msg,
                send_funds,
                label.into(),
                admin,
            )
            .unwrap()
    }

    /// execute a smart contract and return the result
    pub fn execute_smart<T: Serialize + Debug>(
        &mut self,
        sender: impl Into<String>,
        contract_addr: impl Into<String>,
        msg: &T,
        send_funds: &[Coin],
    ) -> AnyResult<AppResponse> {
        self.app.execute_contract(
            Addr::unchecked(sender.into()),
            Addr::unchecked(contract_addr.into()),
            msg,
            send_funds,
        )
    }

    /// execute a smart contract and expect it to succeed
    pub fn execute_smart_ok<T: Serialize + Debug>(
        &mut self,
        sender: impl Into<String>,
        contract_addr: impl Into<String>,
        msg: &T,
        send_funds: &[Coin],
    ) -> AppResponse {
        self.execute_smart(sender, contract_addr, msg, send_funds)
            .unwrap()
    }

    /// execute a smart contract and return the error
    pub fn execute_smart_err<T: Serialize + Debug, E: Display + Debug + Send + Sync + 'static>(
        &mut self,
        sender: impl Into<String>,
        contract_addr: impl Into<String>,
        msg: &T,
        send_funds: &[Coin],
    ) -> E {
        self.execute_smart(sender, contract_addr, msg, send_funds)
            .unwrap_err()
            .downcast()
            .unwrap()
    }

    /// migrate a smart contract and return the result
    pub fn migrate<T: Serialize + Debug>(
        &mut self,
        sender: impl Into<String>,
        contract_addr: impl Into<String>,
        msg: &T,
        code_id: u64,
    ) -> AnyResult<AppResponse> {
        self.app.migrate_contract(
            Addr::unchecked(sender),
            Addr::unchecked(contract_addr),
            msg,
            code_id,
        )
    }

    /// migrate a smart contract and expect it to succeed
    pub fn migrate_ok<T: Serialize + Debug>(
        &mut self,
        sender: impl Into<String>,
        contract_addr: impl Into<String>,
        msg: &T,
        code_id: u64,
    ) -> AppResponse {
        self.migrate(sender, contract_addr, msg, code_id).unwrap()
    }

    /// migrate a smart contract and return the error
    pub fn migrate_err<T: Serialize + Debug, E: Display + Debug + Send + Sync + 'static>(
        &mut self,
        sender: impl Into<String>,
        contract_addr: impl Into<String>,
        msg: &T,
        code_id: u64,
    ) -> E {
        self.migrate(sender, contract_addr, msg, code_id)
            .unwrap_err()
            .downcast()
            .unwrap()
    }

    /// instantiate a cw20 contract and return its address
    pub fn instantiate_cw20(&mut self, name: &str, initial_balances: Vec<Cw20Coin>) -> Addr {
        self.instantiate(
            self.cw20_base_id,
            OWNER,
            &cw20_base::msg::InstantiateMsg {
                name: name.to_string(),
                symbol: name.to_string(),
                decimals: 6,
                initial_balances,
                mint: None,
                marketing: None,
            },
            &[],
            "cw20",
            None,
        )
    }

    /// propose a single choice proposal and return the proposal module address,
    /// proposal ID, and proposal
    pub fn propose_single_choice<T>(
        &mut self,
        dao: &TestDao<T>,
        proposer: impl Into<String>,
        title: impl Into<String>,
        msgs: Vec<CosmosMsg>,
    ) -> (
        Addr,
        u64,
        dao_proposal_single::proposal::SingleChoiceProposal,
    ) {
        let pre_propose_msg = dao_pre_propose_single::ExecuteMsg::Propose {
            msg: dao_pre_propose_single::ProposeMessage::Propose {
                title: title.into(),
                description: "".to_string(),
                msgs,
                vote: None,
            },
        };

        let (pre_propose_module, proposal_module) = &dao.proposal_modules[0];

        self.execute_smart_ok(
            proposer,
            pre_propose_module.as_ref().unwrap(),
            &pre_propose_msg,
            &[],
        );

        let proposal_id: u64 = self
            .querier()
            .query_wasm_smart(
                proposal_module.clone(),
                &dao_proposal_single::msg::QueryMsg::ProposalCount {},
            )
            .unwrap();

        let proposal = self.get_single_choice_proposal(proposal_module, proposal_id);

        (proposal_module.clone(), proposal_id, proposal)
    }

    /// vote on a single choice proposal
    pub fn vote_single_choice<T>(
        &mut self,
        dao: &TestDao<T>,
        voter: impl Into<String>,
        proposal_id: u64,
        vote: dao_voting::voting::Vote,
    ) {
        self.execute_smart_ok(
            voter,
            &dao.proposal_modules[0].1,
            &dao_proposal_single::msg::ExecuteMsg::Vote {
                proposal_id,
                vote,
                rationale: None,
            },
            &[],
        );
    }

    /// add vote hook to all proposal modules
    pub fn add_vote_hook<T>(&mut self, dao: &TestDao<T>, addr: impl Into<String>) {
        let address = addr.into();
        dao.proposal_modules
            .iter()
            .for_each(|(_, proposal_module)| {
                self.execute_smart_ok(
                    dao.core_addr.clone(),
                    proposal_module.clone(),
                    &dao_proposal_single::msg::ExecuteMsg::AddVoteHook {
                        address: address.clone(),
                    },
                    &[],
                );
            });
    }

    /// set the delegation module for all proposal modules
    pub fn set_delegation_module<T>(&mut self, dao: &TestDao<T>, module: impl Into<String>) {
        let module = module.into();
        dao.proposal_modules
            .iter()
            .for_each(|(_, proposal_module)| {
                self.execute_smart_ok(
                    dao.core_addr.clone(),
                    proposal_module.clone(),
                    &dao_proposal_single::msg::ExecuteMsg::UpdateDelegationModule {
                        module: module.clone(),
                    },
                    &[],
                );
            });
    }
}

/// QUERIES
impl DaoTestingSuiteBase {
    /// get the app querier
    pub fn querier(&self) -> QuerierWrapper<'_> {
        self.app.wrap()
    }

    /// get a single choice proposal
    pub fn get_single_choice_proposal(
        &self,
        proposal_module: impl Into<String>,
        proposal_id: u64,
    ) -> dao_proposal_single::proposal::SingleChoiceProposal {
        self.querier()
            .query_wasm_smart::<dao_proposal_single::query::ProposalResponse>(
                Addr::unchecked(proposal_module),
                &dao_proposal_single::msg::QueryMsg::Proposal { proposal_id },
            )
            .unwrap()
            .proposal
    }

    /// assert vote count on single choice proposal
    pub fn assert_single_choice_votes_count(
        &self,
        proposal_module: impl Into<String>,
        proposal_id: u64,
        vote: dao_voting::voting::Vote,
        count: impl Into<Uint128>,
    ) {
        let proposal = self.get_single_choice_proposal(proposal_module, proposal_id);
        assert_eq!(proposal.votes.get(vote), count.into());
    }

    /// assert status on single choice proposal
    pub fn assert_single_choice_status(
        &self,
        proposal_module: impl Into<String>,
        proposal_id: u64,
        status: dao_voting::status::Status,
    ) {
        let proposal = self.get_single_choice_proposal(proposal_module, proposal_id);
        assert_eq!(proposal.status, status);
    }
}
