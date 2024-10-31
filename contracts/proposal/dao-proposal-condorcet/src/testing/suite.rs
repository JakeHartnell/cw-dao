use cosmwasm_std::{coins, to_json_binary, Addr, BankMsg, CosmosMsg, Decimal};
use cw_multi_test::{next_block, App, Executor};
use cw_utils::Duration;
use dao_interface::{
    state::{Admin, ModuleInstantiateInfo},
    voting::InfoResponse,
};
use dao_testing::contracts::{
    cw4_group_contract, dao_dao_core_contract, dao_proposal_condorcet_contract,
    dao_voting_cw4_contract,
};
use dao_voting::threshold::PercentageThreshold;
use dao_voting_cw4::msg::GroupContract;

use crate::{
    config::{Config, UncheckedConfig},
    contract::{CONTRACT_NAME, CONTRACT_VERSION},
    msg::{Choice, ExecuteMsg, InstantiateMsg, QueryMsg},
    proposal::{ProposalResponse, Status},
    tally::Winner,
};

pub(crate) struct Suite {
    app: App,
    sender: Addr,
    pub condorcet: Addr,
    pub core: Addr,
}

pub(crate) struct SuiteBuilder {
    pub instantiate: InstantiateMsg,
    with_proposal: Option<u32>,
    with_voters: Vec<(String, u64)>,
}

impl Default for SuiteBuilder {
    fn default() -> Self {
        Self {
            instantiate: UncheckedConfig {
                quorum: PercentageThreshold::Percent(Decimal::percent(15)),
                voting_period: Duration::Time(60 * 60 * 24 * 7),
                min_voting_period: Some(Duration::Time(60 * 60 * 24)),
                close_proposals_on_execution_failure: true,
            },
            with_proposal: None,
            with_voters: vec![("sender".to_string(), 10)],
        }
    }
}

impl SuiteBuilder {
    #[allow(clippy::field_reassign_with_default)]
    pub fn with_config(instantiate: UncheckedConfig) -> Self {
        let mut b = Self::default();
        b.instantiate = instantiate;
        b
    }

    pub fn with_proposal(mut self, candidates: u32) -> Self {
        self.with_proposal = Some(candidates);
        self
    }

    pub fn with_voters(mut self, voters: &[(&str, u64)]) -> Self {
        self.with_voters = voters.iter().map(|(a, p)| (a.to_string(), *p)).collect();
        self
    }

    pub fn build(self) -> Suite {
        let initial_members: Vec<_> = self
            .with_voters
            .into_iter()
            .map(|(addr, weight)| cw4::Member { addr, weight })
            .collect();
        let sender = Addr::unchecked(&initial_members[0].addr);

        let mut app = App::default();
        let condorcet_id = app.store_code(dao_proposal_condorcet_contract());
        let core_id = app.store_code(dao_dao_core_contract());
        let cw4_id = app.store_code(cw4_group_contract());
        let cw4_voting_id = app.store_code(dao_voting_cw4_contract());

        let core_instantiate = dao_interface::msg::InstantiateMsg {
            admin: None,
            name: "core module".to_string(),
            description: "core module".to_string(),
            image_url: Some("https://moonphase.is/image.svg".to_string()),
            automatically_add_cw20s: false,
            automatically_add_cw721s: false,
            voting_module_instantiate_info: ModuleInstantiateInfo {
                code_id: cw4_voting_id,
                msg: to_json_binary(&dao_voting_cw4::msg::InstantiateMsg {
                    group_contract: GroupContract::New {
                        cw4_group_code_id: cw4_id,
                        initial_members,
                    },
                })
                .unwrap(),
                admin: Some(Admin::CoreModule {}),
                funds: vec![],
                label: "voting module".to_string(),
            },
            proposal_modules_instantiate_info: vec![ModuleInstantiateInfo {
                code_id: condorcet_id,
                msg: to_json_binary(&self.instantiate).unwrap(),
                admin: Some(Admin::CoreModule {}),
                funds: vec![],
                label: "condorcet module".to_string(),
            }],
            initial_items: None,
            dao_uri: None,
        };
        let core = app
            .instantiate_contract(
                core_id,
                sender.clone(),
                &core_instantiate,
                &[],
                "core module".to_string(),
                None,
            )
            .unwrap();
        let condorcet: Vec<dao_interface::state::ProposalModule> = app
            .wrap()
            .query_wasm_smart(
                &core,
                &dao_interface::msg::QueryMsg::ProposalModules {
                    start_after: None,
                    limit: None,
                },
            )
            .unwrap();
        let condorcet = condorcet.into_iter().next().unwrap().address;

        app.update_block(next_block);

        let mut suite = Suite {
            app,
            sender,
            condorcet,
            core,
        };

        let next_id = suite.query_next_proposal_id();
        assert_eq!(next_id, 1);

        if let Some(candidates) = self.with_proposal {
            suite
                .propose(
                    suite.sender(),
                    (0..candidates)
                        .map(|_| vec![unimportant_message()])
                        .collect(),
                )
                .unwrap();
            let next_id = suite.query_next_proposal_id();
            assert_eq!(next_id, 2);
        }

        let dao = suite.query_dao();
        assert_eq!(dao, suite.core);
        let info = suite.query_info();
        assert_eq!(info.info.version, CONTRACT_VERSION);
        assert_eq!(info.info.contract, CONTRACT_NAME);

        suite
    }
}

impl Suite {
    pub fn block_height(&self) -> u64 {
        self.app.block_info().height
    }

    pub fn a_day_passes(&mut self) {
        self.app
            .update_block(|b| b.time = b.time.plus_seconds(60 * 60 * 24))
    }

    pub fn a_week_passes(&mut self) {
        self.a_day_passes();
        self.a_day_passes();
        self.a_day_passes();
        self.a_day_passes();
        self.a_day_passes();
        self.a_day_passes();
        self.a_day_passes();
    }

    pub fn sender(&self) -> Addr {
        self.sender.clone()
    }
}

// query
impl Suite {
    pub fn query_config(&self) -> Config {
        self.app
            .wrap()
            .query_wasm_smart(&self.condorcet, &QueryMsg::Config {})
            .unwrap()
    }

    pub fn query_proposal(&self, id: u32) -> ProposalResponse {
        self.app
            .wrap()
            .query_wasm_smart(&self.condorcet, &QueryMsg::Proposal { id })
            .unwrap()
    }

    pub fn query_winner_and_status(&self, id: u32) -> (Winner, Status) {
        let q = self.query_proposal(id);
        (q.tally.winner, q.proposal.last_status())
    }

    pub fn query_next_proposal_id(&self) -> u32 {
        self.app
            .wrap()
            .query_wasm_smart(&self.condorcet, &QueryMsg::NextProposalId {})
            .unwrap()
    }

    pub fn query_dao(&self) -> Addr {
        self.app
            .wrap()
            .query_wasm_smart(&self.condorcet, &QueryMsg::Dao {})
            .unwrap()
    }

    pub fn query_info(&self) -> InfoResponse {
        self.app
            .wrap()
            .query_wasm_smart(&self.condorcet, &QueryMsg::Info {})
            .unwrap()
    }
}

// execute
impl Suite {
    pub fn propose<S: Into<String>>(
        &mut self,
        sender: S,
        choices: Vec<Vec<CosmosMsg>>,
    ) -> anyhow::Result<u32> {
        let id = self.query_next_proposal_id();
        self.app.execute_contract(
            Addr::unchecked(sender),
            self.condorcet.clone(),
            &ExecuteMsg::Propose {
                choices: choices.into_iter().map(|msgs| Choice { msgs }).collect(),
            },
            &[],
        )?;
        Ok(id)
    }

    pub fn vote<S: Into<String>>(
        &mut self,
        sender: S,
        proposal_id: u32,
        vote: Vec<u32>,
    ) -> anyhow::Result<()> {
        self.app
            .execute_contract(
                Addr::unchecked(sender),
                self.condorcet.clone(),
                &ExecuteMsg::Vote { proposal_id, vote },
                &[],
            )
            .map(|_| ())
    }

    pub fn execute<S: Into<String>>(&mut self, sender: S, proposal_id: u32) -> anyhow::Result<()> {
        self.app
            .execute_contract(
                Addr::unchecked(sender),
                self.condorcet.clone(),
                &ExecuteMsg::Execute { proposal_id },
                &[],
            )
            .map(|_| ())
    }

    pub fn close<S: Into<String>>(&mut self, sender: S, proposal_id: u32) -> anyhow::Result<()> {
        self.app
            .execute_contract(
                Addr::unchecked(sender),
                self.condorcet.clone(),
                &ExecuteMsg::Close { proposal_id },
                &[],
            )
            .map(|_| ())
    }
}

pub fn unimportant_message() -> CosmosMsg {
    BankMsg::Send {
        to_address: "someone".to_string(),
        amount: coins(10, "something"),
    }
    .into()
}
