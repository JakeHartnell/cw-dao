use cosmwasm_std::{to_json_binary, Addr, Coin, Decimal, Empty, Uint128, WasmMsg};
use cw721_base::{
    msg::{
        ExecuteMsg as Cw721ExecuteMsg, InstantiateMsg as Cw721InstantiateMsg,
        QueryMsg as Cw721QueryMsg,
    },
    MinterResponse,
};
use cw_utils::Duration;
use dao_interface::{
    msg::QueryMsg as DaoQueryMsg,
    state::{Admin, ModuleInstantiateInfo},
};
use dao_testing::test_tube::{cw721_base::Cw721Base, dao_dao_core::DaoCore};
use dao_voting::{
    pre_propose::PreProposeInfo,
    threshold::{ActiveThreshold, PercentageThreshold, Threshold},
};
use osmosis_test_tube::{Account, OsmosisTestApp, RunnerError};

use crate::{
    msg::{InstantiateMsg, NftContract, QueryMsg},
    state::Config,
    testing::test_tube_env::Cw721VotingContract,
};

use super::test_tube_env::{TestEnv, TestEnvBuilder};

#[test]
fn test_full_integration_with_factory() {
    let app = OsmosisTestApp::new();
    let env = TestEnvBuilder::new();

    // Setup defaults to creating a NFT DAO with the factory contract
    // This does not use funds when instantiating the NFT contract.
    // We will test that below.
    let TestEnv {
        vp_contract,
        proposal_single,
        custom_factory,
        accounts,
        cw721,
        ..
    } = env.setup(&app);

    // Test instantiating a DAO with a factory contract that requires funds
    let msg = dao_interface::msg::InstantiateMsg {
        dao_uri: None,
        admin: None,
        name: "DAO DAO".to_string(),
        description: "A DAO that makes DAO tooling".to_string(),
        image_url: None,
        automatically_add_cw20s: false,
        automatically_add_cw721s: false,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: vp_contract.code_id,
            msg: to_json_binary(&InstantiateMsg {
                nft_contract: NftContract::Factory(
                    to_json_binary(&WasmMsg::Execute {
                        contract_addr: custom_factory.contract_addr.clone(),
                        msg: to_json_binary(
                            &dao_test_custom_factory::msg::ExecuteMsg::NftFactoryWithFunds {
                                code_id: cw721.code_id,
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
                        funds: vec![Coin {
                            amount: Uint128::new(1000),
                            denom: "uosmo".to_string(),
                        }],
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
            funds: vec![Coin {
                amount: Uint128::new(1000),
                denom: "uosmo".to_string(),
            }],
            label: "DAO DAO Voting Module".to_string(),
        },
        proposal_modules_instantiate_info: vec![ModuleInstantiateInfo {
            code_id: proposal_single.code_id,
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
            })
            .unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "DAO DAO Proposal Module".to_string(),
        }],
        initial_items: None,
    };

    // Instantiating without funds fails
    let err = DaoCore::new(&app, &msg, &accounts[0], &[]).unwrap_err();

    // Error is insufficient funds as no funds were sent
    assert_eq!(
        RunnerError::ExecuteError {
            msg: "failed to execute message; message index: 0: dispatch: submessages: spendable balance  is smaller than 1000uosmo: insufficient funds".to_string()
        },
        err
    );

    // Instantiate DAO succeeds with funds
    let dao = DaoCore::new(
        &app,
        &msg,
        &accounts[0],
        &[Coin {
            amount: Uint128::new(1000),
            denom: "uosmo".to_string(),
        }],
    )
    .unwrap();

    let vp_addr: Addr = dao.query(&DaoQueryMsg::VotingModule {}).unwrap();
    let vp_contract =
        Cw721VotingContract::new_with_values(&app, vp_contract.code_id, vp_addr.to_string())
            .unwrap();

    let vp_config: Config = vp_contract.query(&QueryMsg::Config {}).unwrap();
    let cw721_contract =
        Cw721Base::new_with_values(&app, cw721.code_id, vp_config.nft_address.to_string()).unwrap();

    // Check DAO was initialized to minter
    let minter: MinterResponse = cw721_contract.query(&Cw721QueryMsg::Minter {}).unwrap();
    assert_eq!(minter.minter, Some(dao.contract_addr.to_string()));
}
