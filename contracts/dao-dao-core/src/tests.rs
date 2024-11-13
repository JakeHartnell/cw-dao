use crate::{
    contract::{derive_proposal_module_prefix, migrate, CONTRACT_NAME, CONTRACT_VERSION},
    state::PROPOSAL_MODULES,
    ContractError,
};
use abstract_cw20::msg::Cw20ExecuteMsgFns;
use abstract_cw_plus_interface::cw20_base::Cw20Base;
use v1::DaoDaoCoreV1;

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    from_json,
    testing::{mock_dependencies, mock_env},
    to_json_binary, Addr, CosmosMsg, Empty, Storage, Uint128, WasmMsg,
};
use cw2::{set_contract_version, ContractVersion};
use cw_orch::prelude::*;

use cw_storage_plus::{Item, Map};
use cw_utils::{Duration, Expiration};
use dao_cw_orch::Cw721Base;
use dao_cw_orch::{DaoDaoCore, DaoProposalSudo, DaoVotingCw20Balance};
use dao_interface::CoreExecuteMsgFns;
use dao_interface::CoreQueryMsgFns;
use dao_interface::{
    msg::{ExecuteMsg, InitialItem, InstantiateMsg, MigrateMsg},
    query::{
        AdminNominationResponse, Cw20BalanceResponse, DumpStateResponse, GetItemResponse,
        PauseInfoResponse, ProposalModuleCountResponse, SubDao,
    },
    state::{Admin, Config, ModuleInstantiateInfo, ProposalModule, ProposalModuleStatus},
    voting::{InfoResponse, VotingPowerAtHeightResponse},
};
use dao_proposal_sudo::msg::ExecuteMsgFns as _;
use dao_voting_cw20_balance::msg::QueryMsgFns;

pub fn assert_contains(e: impl std::fmt::Debug, el: impl ToString) {
    assert!(format!("{:?}", e).contains(&el.to_string()))
}

pub mod v1 {
    use cw_orch::{interface, prelude::*};

    use cw_core_v1::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};

    #[interface(InstantiateMsg, ExecuteMsg, QueryMsg, MigrateMsg)]
    pub struct DaoDaoCoreV1;

    impl<Chain> Uploadable for DaoDaoCoreV1<Chain> {
        /// Return the path to the wasm file corresponding to the contract
        fn wasm(_chain: &ChainInfoOwned) -> WasmPath {
            artifacts_dir_from_workspace!()
                .find_wasm_path("dao_dao_core")
                .unwrap()
        }
        /// Returns a CosmWasm contract wrapper
        fn wrapper() -> Box<dyn MockContract<Empty>> {
            use cw_core_v1::contract;
            Box::new(
                ContractWrapper::new(contract::execute, contract::instantiate, contract::query)
                    .with_reply(contract::reply)
                    .with_migrate(contract::migrate),
            )
        }
    }
}

fn test_instantiate_with_n_gov_modules(n: usize) {
    let mock = MockBech32::new("mock");
    let cw20 = Cw20Base::new("cw20", mock.clone());
    let gov = DaoDaoCore::new("dao-core", mock.clone());
    cw20.upload().unwrap();
    let cw20_id = cw20.code_id().unwrap();
    gov.upload().unwrap();

    let cw20_instantiate = cw20_base::msg::InstantiateMsg {
        name: "DAO".to_string(),
        symbol: "DAO".to_string(),
        decimals: 6,
        initial_balances: vec![],
        mint: None,
        marketing: None,
    };
    let instantiate = InstantiateMsg {
        dao_uri: None,
        admin: None,
        name: "DAO DAO".to_string(),
        description: "A DAO that builds DAOs.".to_string(),
        image_url: None,
        automatically_add_cw20s: true,
        automatically_add_cw721s: true,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: cw20_id,
            msg: to_json_binary(&cw20_instantiate).unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "voting module".to_string(),
        },
        proposal_modules_instantiate_info: (0..n)
            .map(|n| ModuleInstantiateInfo {
                code_id: cw20_id,
                msg: to_json_binary(&cw20_instantiate).unwrap(),
                admin: Some(Admin::CoreModule {}),
                funds: vec![],
                label: format!("governance module {n}"),
            })
            .collect(),
        initial_items: None,
    };
    gov.instantiate(&instantiate, None, None).unwrap();

    let state = gov.dump_state().unwrap();

    assert_eq!(
        state.config,
        Config {
            dao_uri: None,
            name: "DAO DAO".to_string(),
            description: "A DAO that builds DAOs.".to_string(),
            image_url: None,
            automatically_add_cw20s: true,
            automatically_add_cw721s: true,
        }
    );

    assert_eq!(state.proposal_modules.len(), n);

    assert_eq!(state.active_proposal_module_count, n as u32);
    assert_eq!(state.total_proposal_module_count, n as u32);
}

#[test]
#[should_panic(expected = "Execution would result in no proposal modules being active.")]
fn test_instantiate_with_zero_gov_modules() {
    test_instantiate_with_n_gov_modules(0)
}

#[test]
fn test_valid_instantiate() {
    let module_counts = [1, 2, 200];
    for count in module_counts {
        test_instantiate_with_n_gov_modules(count)
    }
}

#[test]
#[should_panic(
    expected = "Error parsing into type abstract_cw20_base::msg::InstantiateMsg: Invalid type"
)]
fn test_instantiate_with_submessage_failure() {
    let mock = MockBech32::new("mock");
    let cw20 = Cw20Base::new("cw20", mock.clone());
    let gov = DaoDaoCore::new("dao-core", mock.clone());
    cw20.upload().unwrap();
    let cw20_id = cw20.code_id().unwrap();
    gov.upload().unwrap();

    let cw20_instantiate = cw20_base::msg::InstantiateMsg {
        name: "DAO".to_string(),
        symbol: "DAO".to_string(),
        decimals: 6,
        initial_balances: vec![],
        mint: None,
        marketing: None,
    };

    let mut governance_modules = (0..3)
        .map(|n| ModuleInstantiateInfo {
            code_id: cw20_id,
            msg: to_json_binary(&cw20_instantiate).unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: format!("governance module {n}"),
        })
        .collect::<Vec<_>>();
    governance_modules.push(ModuleInstantiateInfo {
        code_id: cw20_id,
        msg: to_json_binary("bad").unwrap(),
        admin: Some(Admin::CoreModule {}),
        funds: vec![],
        label: "I have a bad instantiate message".to_string(),
    });
    governance_modules.push(ModuleInstantiateInfo {
        code_id: cw20_id,
        msg: to_json_binary(&cw20_instantiate).unwrap(),
        admin: Some(Admin::CoreModule {}),
        funds: vec![],
        label: "Everybody knowing
that goodness is good
makes wickedness."
            .to_string(),
    });

    let instantiate = InstantiateMsg {
        dao_uri: None,
        admin: None,
        name: "DAO DAO".to_string(),
        description: "A DAO that builds DAOs.".to_string(),
        image_url: None,
        automatically_add_cw20s: true,
        automatically_add_cw721s: true,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: cw20_id,
            msg: to_json_binary(&cw20_instantiate).unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "voting module".to_string(),
        },
        proposal_modules_instantiate_info: governance_modules,
        initial_items: None,
    };

    gov.instantiate(&instantiate, None, None).unwrap();
}

#[test]
fn test_update_config() -> cw_orch::anyhow::Result<()> {
    let mock = MockBech32::new("mock");
    let gov_mod = DaoProposalSudo::new("proposal", mock.clone());
    let gov = DaoDaoCore::new("dao-core", mock.clone());
    gov_mod.upload()?;
    let govmod_id = gov_mod.code_id()?;
    gov.upload()?;

    let govmod_instantiate = dao_proposal_sudo::msg::InstantiateMsg {
        root: mock.sender_addr().to_string(),
    };

    let gov_instantiate = InstantiateMsg {
        dao_uri: None,
        admin: None,
        name: "DAO DAO".to_string(),
        description: "A DAO that builds DAOs.".to_string(),
        image_url: None,
        automatically_add_cw20s: true,
        automatically_add_cw721s: true,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: govmod_id,
            msg: to_json_binary(&govmod_instantiate)?,
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "voting module".to_string(),
        },
        proposal_modules_instantiate_info: vec![ModuleInstantiateInfo {
            code_id: govmod_id,
            msg: to_json_binary(&govmod_instantiate)?,
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "voting module".to_string(),
        }],
        initial_items: None,
    };

    gov.instantiate(&gov_instantiate, None, None)?;

    let modules = gov.proposal_modules(None, None)?;
    assert_eq!(modules.len(), 1);
    gov_mod.set_address(&modules[0].clone().address);

    let expected_config = Config {
        name: "Root DAO".to_string(),
        description: "We love trees and sudo.".to_string(),
        image_url: Some("https://moonphase.is/image.svg".to_string()),
        automatically_add_cw20s: false,
        automatically_add_cw721s: true,
        dao_uri: Some("https://daostar.one/EIP".to_string()),
    };

    gov_mod.proposal_execute(vec![WasmMsg::Execute {
        contract_addr: gov.address()?.to_string(),
        funds: vec![],
        msg: to_json_binary(&ExecuteMsg::UpdateConfig {
            config: expected_config.clone(),
        })?,
    }
    .into()])?;

    assert_eq!(expected_config, gov.config()?);

    assert_eq!(gov.dao_uri()?.dao_uri, expected_config.dao_uri);
    Ok(())
}

fn test_swap_governance(swaps: Vec<(u32, u32)>) {
    let mock = MockBech32::new("mock");
    let gov_mod = DaoProposalSudo::new("proposal", mock.clone());
    let gov = DaoDaoCore::new("dao-core", mock.clone());
    gov_mod.upload().unwrap();
    let propmod_id = gov_mod.code_id().unwrap();
    gov.upload().unwrap();

    let govmod_instantiate = dao_proposal_sudo::msg::InstantiateMsg {
        root: mock.sender_addr().to_string(),
    };

    let gov_instantiate = InstantiateMsg {
        dao_uri: None,
        admin: None,
        name: "DAO DAO".to_string(),
        description: "A DAO that builds DAOs.".to_string(),
        image_url: None,
        automatically_add_cw20s: true,
        automatically_add_cw721s: true,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: propmod_id,
            msg: to_json_binary(&govmod_instantiate).unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "voting module".to_string(),
        },
        proposal_modules_instantiate_info: vec![ModuleInstantiateInfo {
            code_id: propmod_id,
            msg: to_json_binary(&govmod_instantiate).unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "governance module".to_string(),
        }],
        initial_items: None,
    };

    gov.instantiate(&gov_instantiate, None, None).unwrap();

    let modules = gov.proposal_modules(None, None).unwrap();
    assert_eq!(modules.len(), 1);
    let module_count = gov.proposal_module_count().unwrap();

    assert_eq!(
        module_count,
        ProposalModuleCountResponse {
            active_proposal_module_count: 1,
            total_proposal_module_count: 1,
        }
    );

    let (to_add, to_remove) = swaps
        .iter()
        .cloned()
        .reduce(|(to_add, to_remove), (add, remove)| (to_add + add, to_remove + remove))
        .unwrap_or((0, 0));

    for (add, remove) in swaps {
        let start_modules = gov.proposal_modules(None, None).unwrap();

        let start_modules_active: Vec<ProposalModule> = get_active_modules(&gov);

        get_active_modules(&gov);
        gov_mod.set_address(&start_modules_active[0].address.clone());
        let to_add: Vec<_> = (0..add)
            .map(|n| ModuleInstantiateInfo {
                code_id: propmod_id,
                msg: to_json_binary(&govmod_instantiate).unwrap(),
                admin: Some(Admin::CoreModule {}),
                funds: vec![],
                label: format!("governance module {n}"),
            })
            .collect();

        let to_disable: Vec<_> = start_modules_active
            .iter()
            .rev()
            .take(remove as usize)
            .map(|a| a.address.to_string())
            .collect();

        gov_mod
            .proposal_execute(vec![WasmMsg::Execute {
                contract_addr: gov.address().unwrap().to_string(),
                funds: vec![],
                msg: to_json_binary(&ExecuteMsg::UpdateProposalModules { to_add, to_disable })
                    .unwrap(),
            }
            .into()])
            .unwrap();

        let finish_modules_active = get_active_modules(&gov);

        assert_eq!(
            finish_modules_active.len() as u32,
            start_modules_active.len() as u32 + add - remove
        );
        for module in start_modules
            .clone()
            .into_iter()
            .rev()
            .take(remove as usize)
        {
            assert!(!finish_modules_active.contains(&module))
        }

        let state: DumpStateResponse = gov.dump_state().unwrap();
        assert_eq!(
            state.active_proposal_module_count,
            finish_modules_active.len() as u32
        );

        assert_eq!(
            state.total_proposal_module_count,
            start_modules.len() as u32 + add
        )
    }

    let module_count = gov.proposal_module_count().unwrap();
    assert_eq!(
        module_count,
        ProposalModuleCountResponse {
            active_proposal_module_count: 1 + to_add - to_remove,
            total_proposal_module_count: 1 + to_add,
        }
    );
}

#[test]
fn test_update_governance() {
    test_swap_governance(vec![(1, 1), (5, 0), (0, 5), (0, 0)]);
    test_swap_governance(vec![(1, 1), (1, 1), (1, 1), (1, 1)])
}

#[test]
fn test_add_then_remove_governance() {
    test_swap_governance(vec![(1, 0), (0, 1)])
}

#[test]
#[should_panic(expected = "Execution would result in no proposal modules being active.")]
fn test_swap_governance_bad() {
    test_swap_governance(vec![(1, 1), (0, 1)])
}

#[test]
fn test_removed_modules_can_not_execute() {
    let mock = MockBech32::new("mock");
    let gov_mod = DaoProposalSudo::new("proposal", mock.clone());
    let gov = DaoDaoCore::new("dao-core", mock.clone());
    gov_mod.upload().unwrap();
    let govmod_id = gov_mod.code_id().unwrap();
    gov.upload().unwrap();

    let govmod_instantiate = dao_proposal_sudo::msg::InstantiateMsg {
        root: mock.sender_addr().to_string(),
    };

    let gov_instantiate = InstantiateMsg {
        dao_uri: None,
        admin: None,
        name: "DAO DAO".to_string(),
        description: "A DAO that builds DAOs.".to_string(),
        image_url: None,
        automatically_add_cw20s: true,
        automatically_add_cw721s: true,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: govmod_id,
            msg: to_json_binary(&govmod_instantiate).unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "voting module".to_string(),
        },
        proposal_modules_instantiate_info: vec![ModuleInstantiateInfo {
            code_id: govmod_id,
            msg: to_json_binary(&govmod_instantiate).unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "governance module".to_string(),
        }],
        initial_items: None,
    };

    gov.instantiate(&gov_instantiate, None, None).unwrap();

    let modules = gov.proposal_modules(None, None).unwrap();

    assert_eq!(modules.len(), 1);

    let start_module = modules.into_iter().next().unwrap();
    gov_mod.set_address(&start_module.address);

    let to_add = vec![ModuleInstantiateInfo {
        code_id: govmod_id,
        msg: to_json_binary(&govmod_instantiate).unwrap(),
        admin: Some(Admin::CoreModule {}),
        funds: vec![],
        label: "new governance module".to_string(),
    }];

    let to_disable = vec![start_module.address.to_string()];

    // Swap ourselves out.
    gov_mod
        .proposal_execute(vec![WasmMsg::Execute {
            contract_addr: gov.address().unwrap().to_string(),
            funds: vec![],
            msg: to_json_binary(&ExecuteMsg::UpdateProposalModules { to_add, to_disable }).unwrap(),
        }
        .into()])
        .unwrap();

    let finish_modules_active = get_active_modules(&gov);

    let new_proposal_module = finish_modules_active.into_iter().next().unwrap();

    // Try to add a new module and remove the one we added
    // earlier. This should fail as we have been removed.
    let to_add = vec![ModuleInstantiateInfo {
        code_id: govmod_id,
        msg: to_json_binary(&govmod_instantiate).unwrap(),
        admin: Some(Admin::CoreModule {}),
        funds: vec![],
        label: "new governance module".to_string(),
    }];
    let to_disable = vec![new_proposal_module.address.to_string()];

    let err = gov_mod
        .proposal_execute(vec![WasmMsg::Execute {
            contract_addr: gov.address().unwrap().to_string(),
            funds: vec![],
            msg: to_json_binary(&ExecuteMsg::UpdateProposalModules {
                to_add: to_add.clone(),
                to_disable: to_disable.clone(),
            })
            .unwrap(),
        }
        .into()])
        .unwrap_err();

    assert_contains(
        err,
        ContractError::ModuleDisabledCannotExecute {
            address: Addr::unchecked(""),
        },
    );

    // Check that the enabled query works.
    let enabled_modules = gov.active_proposal_modules(None, None).unwrap();

    assert_eq!(enabled_modules, vec![new_proposal_module.clone()]);

    // The new proposal module should be able to perform actions.
    gov_mod.set_address(&new_proposal_module.address);
    gov_mod
        .proposal_execute(vec![WasmMsg::Execute {
            contract_addr: gov.address().unwrap().to_string(),
            funds: vec![],
            msg: to_json_binary(&ExecuteMsg::UpdateProposalModules { to_add, to_disable }).unwrap(),
        }
        .into()])
        .unwrap();
}

#[test]
fn test_module_already_disabled() {
    let mock = MockBech32::new("mock");
    let gov_mod = DaoProposalSudo::new("proposal", mock.clone());
    let gov = DaoDaoCore::new("dao-core", mock.clone());
    gov_mod.upload().unwrap();
    let govmod_id = gov_mod.code_id().unwrap();
    gov.upload().unwrap();

    let govmod_instantiate = dao_proposal_sudo::msg::InstantiateMsg {
        root: mock.sender_addr().to_string(),
    };

    let gov_instantiate = InstantiateMsg {
        dao_uri: None,
        admin: None,
        name: "DAO DAO".to_string(),
        description: "A DAO that builds DAOs.".to_string(),
        image_url: None,
        automatically_add_cw20s: true,
        automatically_add_cw721s: true,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: govmod_id,
            msg: to_json_binary(&govmod_instantiate).unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "voting module".to_string(),
        },
        proposal_modules_instantiate_info: vec![ModuleInstantiateInfo {
            code_id: govmod_id,
            msg: to_json_binary(&govmod_instantiate).unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "governance module".to_string(),
        }],
        initial_items: None,
    };

    gov.instantiate(&gov_instantiate, None, None).unwrap();
    let modules = gov.proposal_modules(None, None).unwrap();
    assert_eq!(modules.len(), 1);

    let start_module = modules.into_iter().next().unwrap();
    gov_mod.set_address(&start_module.address);

    let to_disable = vec![
        start_module.address.to_string(),
        start_module.address.to_string(),
    ];

    let err = gov_mod
        .proposal_execute(vec![WasmMsg::Execute {
            contract_addr: gov.address().unwrap().to_string(),
            funds: vec![],
            msg: to_json_binary(&ExecuteMsg::UpdateProposalModules {
                to_add: vec![ModuleInstantiateInfo {
                    code_id: govmod_id,
                    msg: to_json_binary(&govmod_instantiate).unwrap(),
                    admin: Some(Admin::CoreModule {}),
                    funds: vec![],
                    label: "governance module".to_string(),
                }],
                to_disable,
            })
            .unwrap(),
        }
        .into()])
        .unwrap_err();

    assert_contains(
        err,
        ContractError::ModuleAlreadyDisabled {
            address: start_module.address,
        },
    );
}

#[test]
fn test_swap_voting_module() {
    let mock = MockBech32::new("mock");
    let gov_mod = DaoProposalSudo::new("proposal", mock.clone());
    let gov = DaoDaoCore::new("dao-core", mock.clone());
    gov_mod.upload().unwrap();
    let govmod_id = gov_mod.code_id().unwrap();
    gov.upload().unwrap();

    let govmod_instantiate = dao_proposal_sudo::msg::InstantiateMsg {
        root: mock.sender_addr().to_string(),
    };

    let gov_instantiate = InstantiateMsg {
        dao_uri: None,
        admin: None,
        name: "DAO DAO".to_string(),
        description: "A DAO that builds DAOs.".to_string(),
        image_url: None,
        automatically_add_cw20s: true,
        automatically_add_cw721s: true,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: govmod_id,
            msg: to_json_binary(&govmod_instantiate).unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "voting module".to_string(),
        },
        proposal_modules_instantiate_info: vec![ModuleInstantiateInfo {
            code_id: govmod_id,
            msg: to_json_binary(&govmod_instantiate).unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "governance module".to_string(),
        }],
        initial_items: None,
    };

    gov.instantiate(&gov_instantiate, None, None).unwrap();
    let modules = gov.proposal_modules(None, None).unwrap();
    assert_eq!(modules.len(), 1);
    gov_mod.set_address(&modules[0].address);

    let voting_addr = gov.voting_module().unwrap();

    gov_mod
        .proposal_execute(vec![WasmMsg::Execute {
            contract_addr: gov.address().unwrap().to_string(),
            funds: vec![],
            msg: to_json_binary(&ExecuteMsg::UpdateVotingModule {
                module: ModuleInstantiateInfo {
                    code_id: govmod_id,
                    msg: to_json_binary(&govmod_instantiate).unwrap(),
                    admin: Some(Admin::CoreModule {}),
                    funds: vec![],
                    label: "voting module".to_string(),
                },
            })
            .unwrap(),
        }
        .into()])
        .unwrap();

    assert_ne!(gov.voting_module().unwrap(), voting_addr);
}

fn test_unauthorized<Chain: CwEnv>(gov: &DaoDaoCore<Chain>, msg: ExecuteMsg) {
    let err = gov.execute(&msg, None).unwrap_err();

    assert_contains(err, ContractError::Unauthorized {});
}

#[test]
fn test_permissions() {
    let mock = MockBech32::new("mock");
    let gov_mod = DaoProposalSudo::new("proposal", mock.clone());
    let gov = DaoDaoCore::new("dao-core", mock.clone());
    gov_mod.upload().unwrap();
    let govmod_id = gov_mod.code_id().unwrap();
    gov.upload().unwrap();

    let govmod_instantiate = dao_proposal_sudo::msg::InstantiateMsg {
        root: mock.sender_addr().to_string(),
    };

    let gov_instantiate = InstantiateMsg {
        dao_uri: None,
        admin: None,
        name: "DAO DAO".to_string(),
        description: "A DAO that builds DAOs.".to_string(),
        image_url: None,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: govmod_id,
            msg: to_json_binary(&govmod_instantiate).unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "voting module".to_string(),
        },
        proposal_modules_instantiate_info: vec![ModuleInstantiateInfo {
            code_id: govmod_id,
            msg: to_json_binary(&govmod_instantiate).unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "governance module".to_string(),
        }],
        initial_items: None,
        automatically_add_cw20s: true,
        automatically_add_cw721s: true,
    };

    gov.instantiate(&gov_instantiate, None, None).unwrap();

    test_unauthorized(
        &gov,
        ExecuteMsg::UpdateVotingModule {
            module: ModuleInstantiateInfo {
                code_id: govmod_id,
                msg: to_json_binary(&govmod_instantiate).unwrap(),
                admin: Some(Admin::CoreModule {}),
                funds: vec![],
                label: "voting module".to_string(),
            },
        },
    );

    test_unauthorized(
        &gov,
        ExecuteMsg::UpdateProposalModules {
            to_add: vec![],
            to_disable: vec![],
        },
    );

    test_unauthorized(
        &gov,
        ExecuteMsg::UpdateConfig {
            config: Config {
                dao_uri: None,
                name: "Evil config.".to_string(),
                description: "👿".to_string(),
                image_url: None,
                automatically_add_cw20s: true,
                automatically_add_cw721s: true,
            },
        },
    );
}

fn do_standard_instantiate(
    auto_add: bool,
    admin: bool,
) -> (
    DaoDaoCore<MockBech32>,
    DaoProposalSudo<MockBech32>,
    MockBech32,
    Option<Addr>,
) {
    let mock = MockBech32::new("mock");
    let gov_mod = DaoProposalSudo::new("proposal", mock.clone());
    let voting = DaoVotingCw20Balance::new("dao-voting", mock.clone());
    let mut gov = DaoDaoCore::new("dao-core", mock.clone());
    let cw20 = Cw20Base::new("cw20", mock.clone());

    gov_mod.upload().unwrap();
    voting.upload().unwrap();
    gov.upload().unwrap();
    cw20.upload().unwrap();

    let govmod_instantiate = dao_proposal_sudo::msg::InstantiateMsg {
        root: mock.sender_addr().to_string(),
    };
    let voting_instantiate = dao_voting_cw20_balance::msg::InstantiateMsg {
        token_info: dao_voting_cw20_balance::msg::TokenInfo::New {
            code_id: cw20.code_id().unwrap(),
            label: "DAO DAO voting".to_string(),
            name: "DAO DAO".to_string(),
            symbol: "DAO".to_string(),
            decimals: 6,
            initial_balances: vec![cw20::Cw20Coin {
                address: mock.sender_addr().to_string(),
                amount: Uint128::from(2u64),
            }],
            marketing: None,
        },
    };
    let admin = admin.then(|| mock.addr_make("admin"));

    let gov_instantiate = InstantiateMsg {
        dao_uri: None,
        admin: admin.as_ref().map(|a| a.to_string()),
        name: "DAO DAO".to_string(),
        description: "A DAO that builds DAOs.".to_string(),
        image_url: None,
        automatically_add_cw20s: auto_add,
        automatically_add_cw721s: auto_add,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: voting.code_id().unwrap(),
            msg: to_json_binary(&voting_instantiate).unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "voting module".to_string(),
        },
        proposal_modules_instantiate_info: vec![ModuleInstantiateInfo {
            code_id: gov_mod.code_id().unwrap(),
            msg: to_json_binary(&govmod_instantiate).unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "governance module".to_string(),
        }],
        initial_items: None,
    };

    gov.instantiate(&gov_instantiate, None, None).unwrap();

    let proposal_modules = gov.proposal_modules(None, None).unwrap();
    assert_eq!(proposal_modules.len(), 1);
    let proposal_module = proposal_modules.into_iter().next().unwrap();
    gov_mod.set_address(&proposal_module.address);

    if admin.is_none() {
        gov = gov.call_as(&gov.address().unwrap());
    }

    (gov, gov_mod, mock, admin)
}

#[test]
fn test_admin_permissions() {
    let (core, proposal, mock, _) = do_standard_instantiate(true, false);

    let random = mock.addr_make("random");
    let start_height = mock.block_info().unwrap().height;

    // Random address can't call ExecuteAdminMsgs
    core.call_as(&random)
        .execute_admin_msgs(vec![WasmMsg::Execute {
            contract_addr: core.address().unwrap().to_string(),
            msg: to_json_binary(&ExecuteMsg::Pause {
                duration: Duration::Height(10),
            })
            .unwrap(),
            funds: vec![],
        }
        .into()])
        .unwrap_err();

    // Proposal module can't call ExecuteAdminMsgs
    core.call_as(&proposal.address().unwrap())
        .execute_admin_msgs(vec![WasmMsg::Execute {
            contract_addr: core.address().unwrap().to_string(),
            msg: to_json_binary(&ExecuteMsg::Pause {
                duration: Duration::Height(10),
            })
            .unwrap(),
            funds: vec![],
        }
        .into()])
        .unwrap_err();

    // Update Admin can't be called by non-admins
    core.call_as(&random)
        .nominate_admin(Some(random.to_string()))
        .unwrap_err();

    // Nominate admin can be called by core contract as no admin was
    // specified so the admin defaulted to the core contract.

    core.call_as(&proposal.address().unwrap())
        .execute_proposal_hook(vec![WasmMsg::Execute {
            contract_addr: core.address().unwrap().to_string(),
            msg: to_json_binary(&ExecuteMsg::Pause {
                duration: Duration::Height(10),
            })
            .unwrap(),
            funds: vec![],
        }
        .into()])
        .unwrap();

    // Instantiate new DAO with an admin
    let (core_with_admin, proposal_with_admin_address, mock, admin) =
        do_standard_instantiate(true, true);
    let admin = admin.unwrap();

    // Non admins still can't call ExecuteAdminMsgs
    core_with_admin
        .call_as(&proposal_with_admin_address.address().unwrap())
        .execute_admin_msgs(vec![WasmMsg::Execute {
            contract_addr: core_with_admin.address().unwrap().to_string(),
            msg: to_json_binary(&ExecuteMsg::Pause {
                duration: Duration::Height(10),
            })
            .unwrap(),
            funds: vec![],
        }
        .into()])
        .unwrap_err();

    // Admin cannot directly pause the DAO
    core_with_admin
        .call_as(&admin)
        .pause(Duration::Height(10))
        .unwrap_err();

    // Random person cannot pause the DAO
    core_with_admin
        .call_as(&random)
        .pause(Duration::Height(10))
        .unwrap_err();

    // Admin can call ExecuteAdminMsgs, here an admin pauses the DAO
    let _res = core_with_admin
        .call_as(&admin)
        .execute_admin_msgs(vec![WasmMsg::Execute {
            contract_addr: core_with_admin.address().unwrap().to_string(),
            msg: to_json_binary(&ExecuteMsg::Pause {
                duration: Duration::Height(10),
            })
            .unwrap(),
            funds: vec![],
        }
        .into()])
        .unwrap();

    // Ensure we are paused for 10 blocks
    assert_eq!(
        core_with_admin.pause_info().unwrap(),
        PauseInfoResponse::Paused {
            expiration: Expiration::AtHeight(start_height + 10)
        }
    );

    // DAO unpauses after 10 blocks
    mock.wait_blocks(11).unwrap();

    // Check we are unpaused
    assert_eq!(
        core_with_admin.pause_info().unwrap(),
        PauseInfoResponse::Unpaused {}
    );

    // Admin pauses DAO again
    let _res = core_with_admin
        .call_as(&admin)
        .execute_admin_msgs(vec![WasmMsg::Execute {
            contract_addr: core_with_admin.address().unwrap().to_string(),
            msg: to_json_binary(&ExecuteMsg::Pause {
                duration: Duration::Height(10),
            })
            .unwrap(),
            funds: vec![],
        }
        .into()])
        .unwrap();

    // DAO with admin cannot unpause itself
    let _res = core_with_admin
        .call_as(&core_with_admin.address().unwrap())
        .unpause()
        .unwrap_err();

    // Random person cannot unpause the DAO
    let _res = core_with_admin.call_as(&random).unpause().unwrap_err();

    // Admin can unpause the DAO directly
    let _res = core_with_admin.call_as(&admin).unpause().unwrap();

    // Check we are unpaused

    assert_eq!(
        core_with_admin.pause_info().unwrap(),
        PauseInfoResponse::Unpaused {}
    );

    // Admin can nominate a new admin.
    let new_admin = mock.addr_make("meow");
    core_with_admin
        .call_as(&admin)
        .nominate_admin(Some(new_admin.to_string()))
        .unwrap();

    assert_eq!(
        core_with_admin.admin_nomination().unwrap(),
        AdminNominationResponse {
            nomination: Some(new_admin.clone())
        }
    );

    // Check that admin has not yet been updated
    assert_eq!(core_with_admin.admin().unwrap(), admin);

    // Only the nominated address may accept the nomination.
    let err = core_with_admin
        .call_as(&random)
        .accept_admin_nomination()
        .unwrap_err();

    assert_contains(err, ContractError::Unauthorized {});

    // Accept the nomination.
    core_with_admin
        .call_as(&new_admin)
        .accept_admin_nomination()
        .unwrap();

    // Check that admin has been updated
    assert_eq!(core_with_admin.admin().unwrap(), new_admin);

    // Check that the pending admin has been cleared.
    assert_eq!(
        core_with_admin.admin_nomination().unwrap(),
        AdminNominationResponse { nomination: None }
    );
}

#[test]
fn test_admin_nomination() {
    let (core, _, mock, admin) = do_standard_instantiate(true, true);

    let admin = admin.unwrap();
    // Check that there is no pending nominations.
    assert_eq!(
        core.admin_nomination().unwrap(),
        AdminNominationResponse { nomination: None }
    );

    // Nominate a new admin.
    let ekez = mock.addr_make("ekez");
    core.call_as(&admin)
        .nominate_admin(Some(ekez.to_string()))
        .unwrap();

    // Check that the nomination is in place.
    assert_eq!(
        core.admin_nomination().unwrap(),
        AdminNominationResponse {
            nomination: Some(ekez.clone())
        }
    );

    // Non-admin can not withdraw.
    let err = core.call_as(&ekez).withdraw_admin_nomination().unwrap_err();
    assert_contains(err, ContractError::Unauthorized {});

    // Admin can withdraw.
    core.call_as(&admin).withdraw_admin_nomination().unwrap();

    // Check that the nomination is withdrawn.
    assert_eq!(
        core.admin_nomination().unwrap(),
        AdminNominationResponse { nomination: None }
    );

    // Can not withdraw if no nomination is pending.
    let err = core
        .call_as(&admin)
        .withdraw_admin_nomination()
        .unwrap_err();

    assert_contains(err, ContractError::NoAdminNomination {});

    // Can not claim nomination b/c it has been withdrawn.
    let err = core.call_as(&admin).accept_admin_nomination().unwrap_err();

    assert_contains(err, ContractError::NoAdminNomination {});

    // Nominate a new admin.
    let meow = mock.addr_make("meow");
    core.call_as(&admin)
        .nominate_admin(Some(meow.to_string()))
        .unwrap();

    // A new nomination can not be created if there is already a
    // pending nomination.
    let err = core
        .call_as(&admin)
        .nominate_admin(Some(ekez.to_string()))
        .unwrap_err();
    assert_contains(err, ContractError::PendingNomination {});

    // Only nominated admin may accept.
    let err = core.call_as(&ekez).accept_admin_nomination().unwrap_err();
    assert_contains(err, ContractError::Unauthorized {});

    core.call_as(&meow).accept_admin_nomination().unwrap();

    // Check that meow is the new admin.
    assert_eq!(core.admin().unwrap(), meow);

    let start_height = mock.block_info().unwrap().height;
    // Check that the new admin can do admin things and the old can not.
    let err = core
        .call_as(&admin)
        .execute_admin_msgs(vec![WasmMsg::Execute {
            contract_addr: core.address().unwrap().to_string(),
            msg: to_json_binary(&ExecuteMsg::Pause {
                duration: Duration::Height(10),
            })
            .unwrap(),
            funds: vec![],
        }
        .into()])
        .unwrap_err();
    assert_contains(err, ContractError::Unauthorized {});

    core.call_as(&meow)
        .execute_admin_msgs(vec![WasmMsg::Execute {
            contract_addr: core.address().unwrap().to_string(),
            msg: to_json_binary(&ExecuteMsg::Pause {
                duration: Duration::Height(10),
            })
            .unwrap(),
            funds: vec![],
        }
        .into()])
        .unwrap();

    assert_eq!(
        core.pause_info().unwrap(),
        PauseInfoResponse::Paused {
            expiration: Expiration::AtHeight(start_height + 10)
        }
    );

    // DAO unpauses after 10 blocks
    mock.wait_blocks(11).unwrap();

    // Remove the admin.
    core.call_as(&meow).nominate_admin(None).unwrap();

    // Check that this has not caused an admin to be nominated.
    assert_eq!(
        core.admin_nomination().unwrap(),
        AdminNominationResponse { nomination: None }
    );

    // Check that admin has been updated. As there was no admin
    // nominated the admin should revert back to the contract address.
    assert_eq!(core.admin().unwrap(), core.address().unwrap());
}

#[test]
fn test_passthrough_voting_queries() {
    let (gov, _, mock, _) = do_standard_instantiate(true, false);

    assert_eq!(
        gov.voting_power_at_height(mock.sender_addr().to_string(), None)
            .unwrap(),
        VotingPowerAtHeightResponse {
            power: Uint128::from(2u64),
            height: mock.block_info().unwrap().height,
        }
    );
}

#[test]
fn test_item_permissions() {
    let (gov, _, mock, _) = do_standard_instantiate(true, false);

    let ekez = mock.addr_make("ekez");
    let err = gov
        .call_as(&ekez)
        .set_item("k".to_string(), "v".to_string())
        .unwrap_err();
    assert_contains(err, ContractError::Unauthorized {});

    let err = gov.call_as(&ekez).remove_item("k".to_string()).unwrap_err();
    assert_contains(err, ContractError::Unauthorized {});
}

#[test]
fn test_add_remove_get() {
    let (gov, _, _mock, _) = do_standard_instantiate(true, false);

    let a = gov.get_item("aaaaa".to_string()).unwrap();
    assert_eq!(a, GetItemResponse { item: None });

    gov.set_item("aaaaakey".to_string(), "aaaaaaddr".to_string())
        .unwrap();
    let a = gov.get_item("aaaaakey".to_string()).unwrap();
    assert_eq!(
        a,
        GetItemResponse {
            item: Some("aaaaaaddr".to_string())
        }
    );

    gov.remove_item("aaaaakey".to_string()).unwrap();
    let a = gov.get_item("aaaaakey".to_string()).unwrap();
    assert_eq!(a, GetItemResponse { item: None });
}

#[test]
#[should_panic(expected = "Key is missing from storage")]
fn test_remove_missing_key() {
    let (gov, _, _, _) = do_standard_instantiate(true, false);
    gov.remove_item("b".to_string()).unwrap();
}

#[test]
fn test_list_items() {
    let mock = MockBech32::new("mock");
    let govmod = DaoProposalSudo::new("proposal", mock.clone());
    let voting = DaoVotingCw20Balance::new("dao-voting", mock.clone());
    let gov = DaoDaoCore::new("dao-core", mock.clone());
    let cw20 = Cw20Base::new("cw20", mock.clone());

    govmod.upload().unwrap();
    voting.upload().unwrap();
    gov.upload().unwrap();
    cw20.upload().unwrap();
    let govmod_instantiate = dao_proposal_sudo::msg::InstantiateMsg {
        root: mock.sender_addr().to_string(),
    };
    let voting_instantiate = dao_voting_cw20_balance::msg::InstantiateMsg {
        token_info: dao_voting_cw20_balance::msg::TokenInfo::New {
            code_id: cw20.code_id().unwrap(),
            label: "DAO DAO voting".to_string(),
            name: "DAO DAO".to_string(),
            symbol: "DAO".to_string(),
            decimals: 6,
            initial_balances: vec![cw20::Cw20Coin {
                address: mock.sender_addr().to_string(),
                amount: Uint128::from(2u64),
            }],
            marketing: None,
        },
    };

    let gov_instantiate = InstantiateMsg {
        dao_uri: None,
        admin: None,
        name: "DAO DAO".to_string(),
        description: "A DAO that builds DAOs.".to_string(),
        image_url: None,
        automatically_add_cw20s: true,
        automatically_add_cw721s: true,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: voting.code_id().unwrap(),
            msg: to_json_binary(&voting_instantiate).unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "voting module".to_string(),
        },
        proposal_modules_instantiate_info: vec![ModuleInstantiateInfo {
            code_id: govmod.code_id().unwrap(),
            msg: to_json_binary(&govmod_instantiate).unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "governance module".to_string(),
        }],
        initial_items: None,
    };

    gov.instantiate(&gov_instantiate, None, None).unwrap();
    let gov = gov.call_as(&gov.address().unwrap());

    gov.set_item("fookey".to_string(), "fooaddr".to_string())
        .unwrap();
    gov.set_item("barkey".to_string(), "baraddr".to_string())
        .unwrap();
    gov.set_item("loremkey".to_string(), "loremaddr".to_string())
        .unwrap();
    gov.set_item("ipsumkey".to_string(), "ipsumaddr".to_string())
        .unwrap();

    // Foo returned as we are only getting one item and items are in
    // decending order.
    let first_item = gov.list_items(Some(1), None).unwrap();
    assert_eq!(first_item.len(), 1);
    assert_eq!(
        first_item[0],
        ("loremkey".to_string(), "loremaddr".to_string())
    );

    let no_items = gov.list_items(Some(0), None).unwrap();
    assert_eq!(no_items.len(), 0);

    // Items are retreived in decending order so asking for foo with
    // no limit ought to give us the barkey k/v. this will be the last item
    // note: the paginate map bound is exclusive, so fookey will be starting point
    let last_item = gov.list_items(None, Some("foo".to_string())).unwrap();

    assert_eq!(last_item.len(), 1);
    assert_eq!(last_item[0], ("barkey".to_string(), "baraddr".to_string()));

    // Items are retreived in decending order so asking for ipsum with
    // 4 limit ought to give us the fookey and barkey k/vs.
    let after_foo_list = gov.list_items(Some(4), Some("ipsum".to_string())).unwrap();
    assert_eq!(after_foo_list.len(), 2);
    assert_eq!(
        after_foo_list,
        vec![
            ("fookey".to_string(), "fooaddr".to_string()),
            ("barkey".to_string(), "baraddr".to_string())
        ]
    );
}

#[test]
fn test_instantiate_with_items() {
    let mock = MockBech32::new("mock");
    let govmod = DaoProposalSudo::new("proposal", mock.clone());
    let voting = DaoVotingCw20Balance::new("dao-voting", mock.clone());
    let gov = DaoDaoCore::new("dao-core", mock.clone());
    let cw20 = Cw20Base::new("cw20", mock.clone());

    govmod.upload().unwrap();
    voting.upload().unwrap();
    gov.upload().unwrap();
    cw20.upload().unwrap();

    let govmod_instantiate = dao_proposal_sudo::msg::InstantiateMsg {
        root: mock.sender_addr().to_string(),
    };
    let voting_instantiate = dao_voting_cw20_balance::msg::InstantiateMsg {
        token_info: dao_voting_cw20_balance::msg::TokenInfo::New {
            code_id: cw20.code_id().unwrap(),
            label: "DAO DAO voting".to_string(),
            name: "DAO DAO".to_string(),
            symbol: "DAO".to_string(),
            decimals: 6,
            initial_balances: vec![cw20::Cw20Coin {
                address: mock.sender_addr().to_string(),
                amount: Uint128::from(2u64),
            }],
            marketing: None,
        },
    };

    let mut initial_items = vec![
        InitialItem {
            key: "item0".to_string(),
            value: "item0_value".to_string(),
        },
        InitialItem {
            key: "item1".to_string(),
            value: "item1_value".to_string(),
        },
        InitialItem {
            key: "item0".to_string(),
            value: "item0_value_override".to_string(),
        },
    ];

    let mut gov_instantiate = InstantiateMsg {
        dao_uri: None,
        admin: None,
        name: "DAO DAO".to_string(),
        description: "A DAO that builds DAOs.".to_string(),
        image_url: None,
        automatically_add_cw20s: true,
        automatically_add_cw721s: true,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: voting.code_id().unwrap(),
            msg: to_json_binary(&voting_instantiate).unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "voting module".to_string(),
        },
        proposal_modules_instantiate_info: vec![ModuleInstantiateInfo {
            code_id: govmod.code_id().unwrap(),
            msg: to_json_binary(&govmod_instantiate).unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "governance module".to_string(),
        }],
        initial_items: Some(initial_items.clone()),
    };

    // Ensure duplicates are dissallowed.
    let err = gov.instantiate(&gov_instantiate, None, None).unwrap_err();
    assert_contains(
        err,
        ContractError::DuplicateInitialItem {
            item: "item0".to_string(),
        },
    );

    initial_items.pop();
    gov_instantiate.initial_items = Some(initial_items);
    let _gov_addr = gov.instantiate(&gov_instantiate, None, None).unwrap();

    // Ensure initial items were added.
    let items = gov.list_items(None, None).unwrap();
    assert_eq!(items.len(), 2);

    // Descending order, so item1 is first.
    assert_eq!(items[1].0, "item0".to_string());
    let get_item0 = gov.get_item("item0".to_string()).unwrap();

    assert_eq!(
        get_item0,
        GetItemResponse {
            item: Some("item0_value".to_string()),
        }
    );

    assert_eq!(items[0].0, "item1".to_string());
    let item1_value = gov.get_item("item1".to_string()).unwrap().item;
    assert_eq!(item1_value, Some("item1_value".to_string()))
}

#[test]
fn test_cw20_receive_auto_add() {
    let (gov, _proposal, mock, _) = do_standard_instantiate(true, false);
    let another_cw20 = Cw20Base::new("another-cw20", mock.clone());
    another_cw20.upload().unwrap();
    another_cw20
        .instantiate(
            &abstract_cw20_base::msg::InstantiateMsg {
                name: "DAO".to_string(),
                symbol: "DAO".to_string(),
                decimals: 6,
                initial_balances: vec![],
                mint: None,
                marketing: None,
            },
            None,
            None,
        )
        .unwrap();

    let voting = DaoVotingCw20Balance::new("dao-voting", mock.clone());
    voting.set_address(&gov.voting_module().unwrap());

    let gov_token = Cw20Base::new("cw20", mock.clone());

    gov_token.set_address(&voting.token_contract().unwrap());
    // Check that the balances query works with no tokens.
    let cw20_balances = gov.cw_20_balances(None, None).unwrap();
    assert_eq!(cw20_balances, vec![]);

    // Send a gov token to the governance contract.
    gov_token
        .send(
            Uint128::new(1),
            gov.address().unwrap().to_string(),
            to_json_binary(&"").unwrap(),
        )
        .unwrap();

    let cw20_list = gov.cw_20_token_list(None, None).unwrap();
    assert_eq!(
        cw20_list,
        vec![gov_token.address().unwrap().to_string().clone()]
    );

    assert_eq!(
        gov.cw_20_balances(None, None).unwrap(),
        vec![Cw20BalanceResponse {
            addr: gov_token.address().unwrap(),
            balance: Uint128::new(1),
        }]
    );

    // Test removing and adding some new ones. Invalid should fail.
    let err = gov
        .update_cw_20_list(
            vec![mock.addr_make("new").to_string()],
            vec![gov_token.address().unwrap().to_string()],
        )
        .unwrap_err();
    println!("{:?}", err);
    assert_contains(&err, "key:");
    assert_contains(err, "not found");

    // Test that non-DAO can not update the list.
    let err = gov
        .call_as(&mock.addr_make("ekez"))
        .update_cw_20_list(vec![], vec![gov_token.address().unwrap().to_string()])
        .unwrap_err();

    assert_contains(err, ContractError::Unauthorized {});

    gov.update_cw_20_list(
        vec![another_cw20.address().unwrap().to_string()],
        vec![gov_token.address().unwrap().to_string()],
    )
    .unwrap();

    let cw20_list = gov.cw_20_token_list(None, None).unwrap();
    assert_eq!(cw20_list, vec![another_cw20.address().unwrap().to_string()]);
}

#[test]
fn test_cw20_receive_no_auto_add() {
    let (gov, _proposal, mock, _) = do_standard_instantiate(false, false);

    let another_cw20 = Cw20Base::new("another-cw20", mock.clone());
    another_cw20.upload().unwrap();
    another_cw20
        .instantiate(
            &abstract_cw20_base::msg::InstantiateMsg {
                name: "DAO".to_string(),
                symbol: "DAO".to_string(),
                decimals: 6,
                initial_balances: vec![],
                mint: None,
                marketing: None,
            },
            None,
            None,
        )
        .unwrap();

    let voting = DaoVotingCw20Balance::new("dao-voting", mock.clone());
    voting.set_address(&gov.voting_module().unwrap());

    let gov_token = Cw20Base::new("cw20", mock.clone());
    gov_token.set_address(&voting.token_contract().unwrap());

    // Send a gov token to the governance contract. Should not be
    // added becasue auto add is turned off.
    gov_token
        .send(
            Uint128::new(1),
            gov.address().unwrap().to_string(),
            to_json_binary(&"").unwrap(),
        )
        .unwrap();

    assert_eq!(
        gov.cw_20_token_list(None, None).unwrap(),
        Vec::<Addr>::new()
    );

    gov.update_cw_20_list(
        vec![
            another_cw20.address().unwrap().to_string(),
            gov_token.address().unwrap().to_string(),
        ],
        vec![mock.addr_make("ok to remove non existent").to_string()],
    )
    .unwrap();

    assert_eq!(
        gov.cw_20_token_list(None, None).unwrap(),
        vec![
            gov_token.address().unwrap(),
            another_cw20.address().unwrap(),
        ]
    );
}

#[test]
fn test_cw721_receive() {
    let (gov, _proposal, mock, _) = do_standard_instantiate(true, false);

    let cw721 = Cw721Base::new("cw721", mock.clone());
    cw721.upload().unwrap();
    cw721
        .instantiate(
            &cw721_base::msg::InstantiateMsg {
                name: "ekez".to_string(),
                symbol: "ekez".to_string(),
                minter: mock.sender_addr().to_string(),
            },
            None,
            None,
        )
        .unwrap();

    let another_cw721 = Cw721Base::new("another_cw721", mock.clone());
    another_cw721.set_code_id(cw721.code_id().unwrap());
    another_cw721
        .instantiate(
            &cw721_base::msg::InstantiateMsg {
                name: "ekez".to_string(),
                symbol: "ekez".to_string(),
                minter: mock.sender_addr().to_string(),
            },
            None,
            None,
        )
        .unwrap();

    cw721
        .execute(
            &cw721_base::msg::ExecuteMsg::<Option<Empty>, Empty>::Mint {
                token_id: "ekez".to_string(),
                owner: mock.sender_addr().to_string(),
                token_uri: None,
                extension: None,
            },
            None,
        )
        .unwrap();

    cw721
        .execute(
            &cw721_base::msg::ExecuteMsg::<Option<Empty>, Empty>::SendNft {
                contract: gov.address().unwrap().to_string(),
                token_id: "ekez".to_string(),
                msg: to_json_binary("").unwrap(),
            },
            None,
        )
        .unwrap();

    assert_eq!(
        gov.cw_721_token_list(None, None).unwrap(),
        vec![cw721.address().unwrap().clone()]
    );

    // Try to add an invalid cw721.
    let err = gov
        .update_cw_721_list(
            vec![
                mock.addr_make("new").to_string(),
                cw721.address().unwrap().clone().to_string(),
            ],
            vec![cw721.address().unwrap().clone().to_string()],
        )
        .unwrap_err();

    println!("{:?}", err);
    assert_contains(&err, "key:");
    assert_contains(err, "not found");
    // assert!(matches!(err, ContractError::Std(_)));

    // Test that non-DAO can not update the list.
    let err = gov
        .call_as(&mock.addr_make("ekez"))
        .update_cw_721_list(vec![], vec![cw721.address().unwrap().clone().to_string()])
        .unwrap_err();

    assert_contains(err, ContractError::Unauthorized {});

    // Add a real cw721.
    gov.update_cw_721_list(
        vec![
            cw721.address().unwrap().to_string(),
            another_cw721.address().unwrap().to_string(),
        ],
        vec![cw721.address().unwrap().to_string()],
    )
    .unwrap();

    assert_eq!(
        gov.cw_721_token_list(None, None).unwrap(),
        vec![another_cw721.address().unwrap()]
    );
}

#[test]
fn test_cw721_receive_no_auto_add() {
    let (gov, _proposal, mock, _) = do_standard_instantiate(false, false);

    let cw721 = Cw721Base::new("cw721", mock.clone());
    cw721.upload().unwrap();
    cw721
        .instantiate(
            &cw721_base::msg::InstantiateMsg {
                name: "ekez".to_string(),
                symbol: "ekez".to_string(),
                minter: mock.sender_addr().to_string(),
            },
            None,
            None,
        )
        .unwrap();

    let another_cw721 = Cw721Base::new("another_cw721", mock.clone());
    another_cw721.set_code_id(cw721.code_id().unwrap());
    another_cw721
        .instantiate(
            &cw721_base::msg::InstantiateMsg {
                name: "ekez".to_string(),
                symbol: "ekez".to_string(),
                minter: mock.sender_addr().to_string(),
            },
            None,
            None,
        )
        .unwrap();

    assert_eq!(
        gov.cw_721_token_list(None, None).unwrap(),
        Vec::<Addr>::new()
    );

    // Duplicates OK. Just adds one.
    gov.update_cw_721_list(
        vec![
            another_cw721.address().unwrap().to_string(),
            cw721.address().unwrap().to_string(),
            cw721.address().unwrap().to_string(),
        ],
        vec![],
    )
    .unwrap();

    assert_eq!(
        gov.cw_721_token_list(None, None).unwrap(),
        vec![another_cw721.address().unwrap(), cw721.address().unwrap()]
    );
}

#[test]
fn test_pause() {
    let (gov, _proposal, mock, _) = do_standard_instantiate(false, false);

    let start_height = mock.block_info().unwrap().height;

    let proposal_modules = gov.proposal_modules(None, None).unwrap();
    assert_eq!(proposal_modules.len(), 1);
    let proposal_module = proposal_modules.into_iter().next().unwrap();

    assert_eq!(gov.pause_info().unwrap(), PauseInfoResponse::Unpaused {});

    assert_eq!(
        gov.dump_state().unwrap().pause_info,
        PauseInfoResponse::Unpaused {}
    );

    // DAO is not paused. Check that we can execute things.
    //
    // Tests intentionally use the core address to send these
    // messsages to simulate a worst case scenerio where the core
    // contract has a vulnerability.
    gov.update_config(Config {
        dao_uri: None,
        name: "The Empire Strikes Back".to_string(),
        description: "haha lol we have pwned your DAO".to_string(),
        image_url: None,
        automatically_add_cw20s: true,
        automatically_add_cw721s: true,
    })
    .unwrap();

    // Oh no the DAO is under attack! Quick! Pause the DAO while we
    // figure out what to do!
    let err = gov
        .call_as(&proposal_module.address)
        .pause(Duration::Height(10))
        .unwrap_err();

    // Only the DAO may call this on itself. Proposal modules must use
    // the execute hook.
    assert_contains(err, ContractError::Unauthorized {});
    gov.call_as(&proposal_module.address)
        .execute_proposal_hook(vec![WasmMsg::Execute {
            contract_addr: gov.address().unwrap().to_string(),
            msg: to_json_binary(&ExecuteMsg::Pause {
                duration: Duration::Height(10),
            })
            .unwrap(),
            funds: vec![],
        }
        .into()])
        .unwrap();

    assert_eq!(
        gov.pause_info().unwrap(),
        PauseInfoResponse::Paused {
            expiration: Expiration::AtHeight(start_height + 10)
        }
    );
    assert_eq!(
        gov.dump_state().unwrap().pause_info,
        PauseInfoResponse::Paused {
            expiration: Expiration::AtHeight(start_height + 10)
        }
    );

    // This should actually be allowed to enable the admin to execute
    gov.update_config(Config {
        dao_uri: None,
        name: "The Empire Strikes Back Again".to_string(),
        description: "haha lol we have pwned your DAO again".to_string(),
        image_url: None,
        automatically_add_cw20s: true,
        automatically_add_cw721s: true,
    })
    .unwrap();

    let err = gov
        .call_as(&proposal_module.address)
        .execute_proposal_hook(vec![WasmMsg::Execute {
            contract_addr: gov.address().unwrap().to_string(),
            msg: to_json_binary(&ExecuteMsg::Pause {
                duration: Duration::Height(10),
            })
            .unwrap(),
            funds: vec![],
        }
        .into()])
        .unwrap_err();

    assert_contains(err, ContractError::Paused {});

    mock.wait_blocks(9).unwrap();

    // Still not unpaused.

    let err = gov
        .call_as(&proposal_module.address)
        .execute_proposal_hook(vec![WasmMsg::Execute {
            contract_addr: gov.address().unwrap().to_string(),
            msg: to_json_binary(&ExecuteMsg::Pause {
                duration: Duration::Height(10),
            })
            .unwrap(),
            funds: vec![],
        }
        .into()])
        .unwrap_err();

    assert_contains(err, ContractError::Paused {});

    mock.wait_blocks(1).unwrap();

    assert_eq!(gov.pause_info().unwrap(), PauseInfoResponse::Unpaused {});
    assert_eq!(
        gov.dump_state().unwrap().pause_info,
        PauseInfoResponse::Unpaused {}
    );

    // Now its unpaused so we should be able to pause again.
    gov.call_as(&proposal_module.address)
        .execute_proposal_hook(vec![WasmMsg::Execute {
            contract_addr: gov.address().unwrap().to_string(),
            msg: to_json_binary(&ExecuteMsg::Pause {
                duration: Duration::Height(10),
            })
            .unwrap(),
            funds: vec![],
        }
        .into()])
        .unwrap();

    assert_eq!(
        gov.pause_info().unwrap(),
        PauseInfoResponse::Paused {
            expiration: Expiration::AtHeight(start_height + 20)
        }
    );
    assert_eq!(
        gov.dump_state().unwrap().pause_info,
        PauseInfoResponse::Paused {
            expiration: Expiration::AtHeight(start_height + 20)
        }
    );
}

#[test]
fn test_dump_state_proposal_modules() {
    let (gov, _proposal, _mock, _) = do_standard_instantiate(false, false);
    let proposal_modules = gov.proposal_modules(None, None).unwrap();

    assert_eq!(proposal_modules.len(), 1);
    let proposal_module = proposal_modules.into_iter().next().unwrap();

    let all_state: DumpStateResponse = gov.dump_state().unwrap();
    assert_eq!(all_state.pause_info, PauseInfoResponse::Unpaused {});
    assert_eq!(all_state.proposal_modules.len(), 1);
    assert_eq!(all_state.proposal_modules[0], proposal_module);
}

// Note that this isn't actually testing that we are migrating from the previous version since
// with multitest contract instantiation we can't manipulate storage to the previous version of state before invoking migrate. So if anything,
// this just tests the idempotency of migrate.
#[test]
fn test_migrate_from_compatible() {
    let mock = MockBech32::new("mock");
    let govmod = DaoProposalSudo::new("proposal", mock.clone());
    let voting = DaoVotingCw20Balance::new("dao-voting", mock.clone());
    let gov = DaoDaoCore::new("dao-core", mock.clone());
    let cw20 = Cw20Base::new("cw20", mock.clone());

    govmod.upload().unwrap();
    voting.upload().unwrap();
    gov.upload().unwrap();
    cw20.upload().unwrap();

    let govmod_instantiate = dao_proposal_sudo::msg::InstantiateMsg {
        root: mock.sender_addr().to_string(),
    };
    let voting_instantiate = dao_voting_cw20_balance::msg::InstantiateMsg {
        token_info: dao_voting_cw20_balance::msg::TokenInfo::New {
            code_id: cw20.code_id().unwrap(),
            label: "DAO DAO voting".to_string(),
            name: "DAO DAO".to_string(),
            symbol: "DAO".to_string(),
            decimals: 6,
            initial_balances: vec![cw20::Cw20Coin {
                address: mock.sender_addr().to_string(),
                amount: Uint128::from(2u64),
            }],
            marketing: None,
        },
    };

    // Instantiate the core module with an admin to do migrations.
    let gov_instantiate = InstantiateMsg {
        dao_uri: None,
        admin: None,
        name: "DAO DAO".to_string(),
        description: "A DAO that builds DAOs.".to_string(),
        image_url: None,
        automatically_add_cw20s: false,
        automatically_add_cw721s: false,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: voting.code_id().unwrap(),
            msg: to_json_binary(&voting_instantiate).unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "voting module".to_string(),
        },
        proposal_modules_instantiate_info: vec![ModuleInstantiateInfo {
            code_id: govmod.code_id().unwrap(),
            msg: to_json_binary(&govmod_instantiate).unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "governance module".to_string(),
        }],
        initial_items: None,
    };

    gov.instantiate(&gov_instantiate, Some(&mock.sender_addr()), None)
        .unwrap();

    let state = gov.dump_state().unwrap();

    gov.migrate(&MigrateMsg::FromCompatible {}, gov.code_id().unwrap())
        .unwrap();

    let new_state = gov.dump_state().unwrap();

    assert_eq!(new_state, state);
}

#[test]
fn test_migrate_from_beta() {
    use cw_core_v1 as v1;

    let mock = MockBech32::new("mock");
    let govmod = DaoProposalSudo::new("proposal", mock.clone());
    let voting = DaoVotingCw20Balance::new("dao-voting", mock.clone());
    let gov = DaoDaoCore::new("dao-core", mock.clone());
    let v1_gov = DaoDaoCoreV1::new("dao-core-v1", mock.clone());
    let cw20 = Cw20Base::new("cw20", mock.clone());

    govmod.upload().unwrap();
    voting.upload().unwrap();
    gov.upload().unwrap();
    v1_gov.upload().unwrap();
    cw20.upload().unwrap();

    let proposal_instantiate = dao_proposal_sudo::msg::InstantiateMsg {
        root: mock.sender_addr().to_string(),
    };
    let voting_instantiate = dao_voting_cw20_balance::msg::InstantiateMsg {
        token_info: dao_voting_cw20_balance::msg::TokenInfo::New {
            code_id: cw20.code_id().unwrap(),
            label: "DAO DAO voting".to_string(),
            name: "DAO DAO".to_string(),
            symbol: "DAO".to_string(),
            decimals: 6,
            initial_balances: vec![cw20::Cw20Coin {
                address: mock.sender_addr().to_string(),
                amount: Uint128::from(2u64),
            }],
            marketing: None,
        },
    };

    // Instantiate the core module with an admin to do migrations.
    let v1_core_instantiate = v1::msg::InstantiateMsg {
        admin: None,
        name: "DAO DAO".to_string(),
        description: "A DAO that builds DAOs.".to_string(),
        image_url: None,
        automatically_add_cw20s: false,
        automatically_add_cw721s: false,
        voting_module_instantiate_info: v1::msg::ModuleInstantiateInfo {
            code_id: voting.code_id().unwrap(),
            msg: to_json_binary(&voting_instantiate).unwrap(),
            admin: v1::msg::Admin::CoreContract {},
            label: "voting module".to_string(),
        },
        proposal_modules_instantiate_info: vec![
            v1::msg::ModuleInstantiateInfo {
                code_id: govmod.code_id().unwrap(),
                msg: to_json_binary(&proposal_instantiate).unwrap(),
                admin: v1::msg::Admin::CoreContract {},
                label: "governance module 1".to_string(),
            },
            v1::msg::ModuleInstantiateInfo {
                code_id: govmod.code_id().unwrap(),
                msg: to_json_binary(&proposal_instantiate).unwrap(),
                admin: v1::msg::Admin::CoreContract {},
                label: "governance module 2".to_string(),
            },
        ],
        initial_items: None,
    };

    v1_gov
        .instantiate(&v1_core_instantiate, Some(&mock.sender_addr()), None)
        .unwrap();

    gov.set_address(&v1_gov.address().unwrap());
    gov.migrate(
        &MigrateMsg::FromV1 {
            dao_uri: None,
            params: None,
        },
        gov.code_id().unwrap(),
    )
    .unwrap();

    let new_state = gov.dump_state().unwrap();

    let proposal_modules = new_state.proposal_modules;
    assert_eq!(2, proposal_modules.len());
    for (idx, module) in proposal_modules.iter().enumerate() {
        let prefix = derive_proposal_module_prefix(idx).unwrap();
        assert_eq!(prefix, module.prefix);
        assert_eq!(ProposalModuleStatus::Enabled, module.status);
    }

    // Check that we may not migrate more than once.
    let err = gov
        .migrate(
            &MigrateMsg::FromV1 {
                dao_uri: None,
                params: None,
            },
            gov.code_id().unwrap(),
        )
        .unwrap_err();

    assert_contains(err, ContractError::AlreadyMigrated {})
}

#[test]
fn test_migrate_mock() {
    let mut deps = mock_dependencies();
    let dao_uri: String = "/dao/uri".to_string();
    let msg = MigrateMsg::FromV1 {
        dao_uri: Some(dao_uri.clone()),
        params: None,
    };
    let env = mock_env();

    // Set starting version to v1.
    set_contract_version(&mut deps.storage, CONTRACT_NAME, "0.1.0").unwrap();

    // Write to storage in old proposal module format
    let proposal_modules_key = Addr::unchecked("addr");
    let old_map: Map<Addr, Empty> = Map::new("proposal_modules");
    let path = old_map.key(proposal_modules_key.clone());
    deps.storage.set(&path, &to_json_binary(&Empty {}).unwrap());

    // Write to storage in old config format
    #[cw_serde]
    struct V1Config {
        pub name: String,
        pub description: String,
        pub image_url: Option<String>,
        pub automatically_add_cw20s: bool,
        pub automatically_add_cw721s: bool,
    }

    let v1_config = V1Config {
        name: "core dao".to_string(),
        description: "a dao".to_string(),
        image_url: None,
        automatically_add_cw20s: false,
        automatically_add_cw721s: false,
    };

    let config_item: Item<V1Config> = Item::new("config");
    config_item.save(&mut deps.storage, &v1_config).unwrap();

    // Migrate to v2
    migrate(deps.as_mut(), env, msg).unwrap();

    let new_path = PROPOSAL_MODULES.key(proposal_modules_key);
    let prop_module_bytes = deps.storage.get(&new_path).unwrap();
    let module: ProposalModule = from_json(prop_module_bytes).unwrap();
    assert_eq!(module.address, Addr::unchecked("addr"));
    assert_eq!(module.prefix, derive_proposal_module_prefix(0).unwrap());
    assert_eq!(module.status, ProposalModuleStatus::Enabled {});

    let v2_config_item: Item<Config> = Item::new("config_v2");
    let v2_config = v2_config_item.load(&deps.storage).unwrap();
    assert_eq!(v2_config.dao_uri, Some(dao_uri));
    assert_eq!(v2_config.name, v1_config.name);
    assert_eq!(v2_config.description, v1_config.description);
    assert_eq!(v2_config.image_url, v1_config.image_url);
    assert_eq!(
        v2_config.automatically_add_cw20s,
        v1_config.automatically_add_cw20s
    );
    assert_eq!(
        v2_config.automatically_add_cw721s,
        v1_config.automatically_add_cw721s
    )
}

#[test]
fn test_execute_stargate_msg() {
    let (gov, _proposal, _mock, _) = do_standard_instantiate(true, false);
    let proposal_modules = gov.proposal_modules(None, None).unwrap();

    assert_eq!(proposal_modules.len(), 1);
    let proposal_module = proposal_modules.into_iter().next().unwrap();

    let res = gov
        .call_as(&proposal_module.address)
        .execute_proposal_hook(vec![CosmosMsg::Stargate {
            type_url: "foo_type".to_string(),
            value: to_json_binary("foo_bin").unwrap(),
        }]);

    // TODO: Once cw-multi-test supports executing stargate/ibc messages we can change this test assert
    assert!(res.is_err());
}

#[test]
fn test_module_prefixes() {
    let mock = MockBech32::new("mock");
    let gov_mod = DaoProposalSudo::new("proposal", mock.clone());
    let gov = DaoDaoCore::new("dao-core", mock.clone());
    gov_mod.upload().unwrap();
    let govmod_id = gov_mod.code_id().unwrap();
    gov.upload().unwrap();

    let govmod_instantiate = dao_proposal_sudo::msg::InstantiateMsg {
        root: mock.sender_addr().to_string(),
    };

    let gov_instantiate = InstantiateMsg {
        dao_uri: None,
        admin: None,
        name: "DAO DAO".to_string(),
        description: "A DAO that builds DAOs.".to_string(),
        image_url: None,
        automatically_add_cw20s: true,
        automatically_add_cw721s: true,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: govmod_id,
            msg: to_json_binary(&govmod_instantiate).unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "voting module".to_string(),
        },
        proposal_modules_instantiate_info: vec![
            ModuleInstantiateInfo {
                code_id: govmod_id,
                msg: to_json_binary(&govmod_instantiate).unwrap(),
                admin: Some(Admin::CoreModule {}),
                funds: vec![],
                label: "proposal module 1".to_string(),
            },
            ModuleInstantiateInfo {
                code_id: govmod_id,
                msg: to_json_binary(&govmod_instantiate).unwrap(),
                admin: Some(Admin::CoreModule {}),
                funds: vec![],
                label: "proposal module 2".to_string(),
            },
            ModuleInstantiateInfo {
                code_id: govmod_id,
                msg: to_json_binary(&govmod_instantiate).unwrap(),
                admin: Some(Admin::CoreModule {}),
                funds: vec![],
                label: "proposal module 2".to_string(),
            },
        ],
        initial_items: None,
    };

    gov.instantiate(&gov_instantiate, None, None).unwrap();

    let modules = gov.proposal_modules(None, None).unwrap();
    assert_eq!(modules.len(), 3);

    let module_1 = &modules[0];
    assert_eq!(module_1.status, ProposalModuleStatus::Enabled {});
    assert_eq!(module_1.prefix, "A");
    assert_eq!(&module_1.address, &modules[0].address);

    let module_2 = &modules[1];
    assert_eq!(module_2.status, ProposalModuleStatus::Enabled {});
    assert_eq!(module_2.prefix, "C");
    assert_eq!(&module_2.address, &modules[1].address);

    let module_3 = &modules[2];
    assert_eq!(module_3.status, ProposalModuleStatus::Enabled {});
    assert_eq!(module_3.prefix, "B");
    assert_eq!(&module_3.address, &modules[2].address);
}

fn get_active_modules<Chain: CwEnv>(gov: &DaoDaoCore<Chain>) -> Vec<ProposalModule> {
    let modules = gov.proposal_modules(None, None).unwrap();

    modules
        .into_iter()
        .filter(|module: &ProposalModule| module.status == ProposalModuleStatus::Enabled)
        .collect()
}

#[test]
fn test_add_remove_subdaos() {
    let (gov, _proposal, mock, _) = do_standard_instantiate(false, false);

    test_unauthorized(
        &gov.call_as(&mock.sender_addr()),
        ExecuteMsg::UpdateSubDaos {
            to_add: vec![],
            to_remove: vec![],
        },
    );

    let to_add: Vec<SubDao> = vec![
        SubDao {
            addr: mock.addr_make("subdao001").to_string(),
            charter: None,
        },
        SubDao {
            addr: mock.addr_make("subdao002").to_string(),
            charter: Some("cool charter bro".to_string()),
        },
        SubDao {
            addr: mock.addr_make("subdao005").to_string(),
            charter: None,
        },
        SubDao {
            addr: mock.addr_make("subdao007").to_string(),
            charter: None,
        },
    ];
    let to_remove: Vec<String> = vec![];

    gov.update_sub_daos(to_add, to_remove).unwrap();

    assert_eq!(gov.list_sub_daos(None, None).unwrap().len(), 4);

    let to_remove: Vec<String> = vec![mock.addr_make("subdao005").to_string()];

    gov.update_sub_daos(vec![], to_remove).unwrap();

    let res = gov.list_sub_daos(None, None).unwrap();

    assert_eq!(res.len(), 3);
    let full_result_set: Vec<SubDao> = vec![
        SubDao {
            addr: mock.addr_make("subdao001").to_string(),
            charter: None,
        },
        SubDao {
            addr: mock.addr_make("subdao002").to_string(),
            charter: Some("cool charter bro".to_string()),
        },
        SubDao {
            addr: mock.addr_make("subdao007").to_string(),
            charter: None,
        },
    ];

    assert_eq!(res, full_result_set);
}

#[test]
pub fn test_migrate_update_version() {
    let mut deps = mock_dependencies();
    cw2::set_contract_version(&mut deps.storage, "my-contract", "old-version").unwrap();
    migrate(deps.as_mut(), mock_env(), MigrateMsg::FromCompatible {}).unwrap();
    let version = cw2::get_contract_version(&deps.storage).unwrap();
    assert_eq!(version.version, CONTRACT_VERSION);
    assert_eq!(version.contract, CONTRACT_NAME);
}

#[test]
fn test_query_info() {
    let (gov, _, _, _) = do_standard_instantiate(true, false);
    assert_eq!(
        gov.info().unwrap(),
        InfoResponse {
            info: ContractVersion {
                contract: CONTRACT_NAME.to_string(),
                version: CONTRACT_VERSION.to_string()
            }
        }
    )
}
