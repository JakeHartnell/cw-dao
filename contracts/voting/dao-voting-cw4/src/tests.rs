use cosmwasm_std::{
    testing::{mock_dependencies, mock_env},
    to_json_binary, Addr, CosmosMsg, Uint128, WasmMsg,
};
use cw2::ContractVersion;
use cw_multi_test::{next_block, App, Executor};
use dao_interface::voting::{
    InfoResponse, TotalPowerAtHeightResponse, VotingPowerAtHeightResponse,
};
use dao_testing::contracts::{cw4_group_contract, dao_voting_cw4_contract};

use crate::{
    contract::{migrate, CONTRACT_NAME, CONTRACT_VERSION},
    msg::{GroupContract, InstantiateMsg, MigrateMsg, QueryMsg},
};
use dao_voting_cw4::ContractError;

const DAO_ADDR: &str = "dao";
const ADDR1: &str = "addr1";
const ADDR2: &str = "addr2";
const ADDR3: &str = "addr3";
const ADDR4: &str = "addr4";

fn instantiate_voting(app: &mut App, voting_id: u64, msg: InstantiateMsg) -> Addr {
    app.instantiate_contract(
        voting_id,
        Addr::unchecked(DAO_ADDR),
        &msg,
        &[],
        "voting module",
        None,
    )
    .unwrap()
}

fn setup_test_case(app: &mut App) -> Addr {
    let cw4_id = app.store_code(cw4_group_contract());
    let voting_id = app.store_code(dao_voting_cw4_contract());

    let members = vec![
        cw4::Member {
            addr: ADDR1.to_string(),
            weight: 1,
        },
        cw4::Member {
            addr: ADDR2.to_string(),
            weight: 1,
        },
        cw4::Member {
            addr: ADDR3.to_string(),
            weight: 1,
        },
        cw4::Member {
            addr: ADDR4.to_string(),
            weight: 0,
        },
    ];
    instantiate_voting(
        app,
        voting_id,
        InstantiateMsg {
            group_contract: GroupContract::New {
                cw4_group_code_id: cw4_id,
                initial_members: members,
            },
        },
    )
}

#[test]
fn test_instantiate() {
    let mut app = App::default();
    // Valid instantiate no panics
    let _voting_addr = setup_test_case(&mut app);

    // Instantiate with no members, error
    let voting_id = app.store_code(dao_voting_cw4_contract());
    let cw4_id = app.store_code(cw4_group_contract());
    let msg = InstantiateMsg {
        group_contract: GroupContract::New {
            cw4_group_code_id: cw4_id,
            initial_members: [].into(),
        },
    };
    let _err = app
        .instantiate_contract(
            voting_id,
            Addr::unchecked(DAO_ADDR),
            &msg,
            &[],
            "voting module",
            None,
        )
        .unwrap_err();

    // Instantiate with members but no weight
    let msg = InstantiateMsg {
        group_contract: GroupContract::New {
            cw4_group_code_id: cw4_id,
            initial_members: vec![
                cw4::Member {
                    addr: ADDR1.to_string(),
                    weight: 0,
                },
                cw4::Member {
                    addr: ADDR2.to_string(),
                    weight: 0,
                },
                cw4::Member {
                    addr: ADDR3.to_string(),
                    weight: 0,
                },
            ],
        },
    };
    let _err = app
        .instantiate_contract(
            voting_id,
            Addr::unchecked(DAO_ADDR),
            &msg,
            &[],
            "voting module",
            None,
        )
        .unwrap_err();
}

#[test]
pub fn test_instantiate_existing_contract() {
    let mut app = App::default();

    let voting_id = app.store_code(dao_voting_cw4_contract());
    let cw4_id = app.store_code(cw4_group_contract());

    // Fail with no members.
    let cw4_addr = app
        .instantiate_contract(
            cw4_id,
            Addr::unchecked(DAO_ADDR),
            &cw4_group::msg::InstantiateMsg {
                admin: Some(DAO_ADDR.to_string()),
                members: vec![],
            },
            &[],
            "cw4 group",
            None,
        )
        .unwrap();

    let err: ContractError = app
        .instantiate_contract(
            voting_id,
            Addr::unchecked(DAO_ADDR),
            &InstantiateMsg {
                group_contract: GroupContract::Existing {
                    address: cw4_addr.to_string(),
                },
            },
            &[],
            "voting module",
            None,
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::NoMembers {});

    let cw4_addr = app
        .instantiate_contract(
            cw4_id,
            Addr::unchecked(DAO_ADDR),
            &cw4_group::msg::InstantiateMsg {
                admin: Some(DAO_ADDR.to_string()),
                members: vec![cw4::Member {
                    addr: ADDR1.to_string(),
                    weight: 1,
                }],
            },
            &[],
            "cw4 group",
            None,
        )
        .unwrap();

    // Instantiate with existing contract
    let msg = InstantiateMsg {
        group_contract: GroupContract::Existing {
            address: cw4_addr.to_string(),
        },
    };
    let _err = app
        .instantiate_contract(
            voting_id,
            Addr::unchecked(DAO_ADDR),
            &msg,
            &[],
            "voting module",
            None,
        )
        .unwrap();

    // Update ADDR1's weight to 2
    let msg = cw4_group::msg::ExecuteMsg::UpdateMembers {
        remove: vec![],
        add: vec![cw4::Member {
            addr: ADDR1.to_string(),
            weight: 2,
        }],
    };

    app.execute_contract(Addr::unchecked(DAO_ADDR), cw4_addr.clone(), &msg, &[])
        .unwrap();

    // Same should be true about the groups contract.
    let cw4_power: cw4::MemberResponse = app
        .wrap()
        .query_wasm_smart(
            cw4_addr,
            &cw4::Cw4QueryMsg::Member {
                addr: ADDR1.to_string(),
                at_height: None,
            },
        )
        .unwrap();
    assert_eq!(cw4_power.weight.unwrap(), 2);
}

#[test]
fn test_contract_info() {
    let mut app = App::default();
    let voting_addr = setup_test_case(&mut app);

    let info: InfoResponse = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::Info {})
        .unwrap();
    assert_eq!(
        info,
        InfoResponse {
            info: ContractVersion {
                contract: "crates.io:dao-voting-cw4".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string()
            }
        }
    );

    // Ensure group contract is set
    let _group_contract: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::GroupContract {})
        .unwrap();

    let dao_contract: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr, &QueryMsg::Dao {})
        .unwrap();
    assert_eq!(dao_contract, Addr::unchecked(DAO_ADDR));
}

#[test]
fn test_power_at_height() {
    let mut app = App::default();
    let voting_addr = setup_test_case(&mut app);
    app.update_block(next_block);

    let cw4_addr: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::GroupContract {})
        .unwrap();

    let addr1_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::VotingPowerAtHeight {
                address: ADDR1.to_string(),
                height: None,
            },
        )
        .unwrap();
    assert_eq!(addr1_voting_power.power, Uint128::new(1u128));
    assert_eq!(addr1_voting_power.height, app.block_info().height);

    let total_voting_power: TotalPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::TotalPowerAtHeight { height: None },
        )
        .unwrap();
    assert_eq!(total_voting_power.power, Uint128::new(3u128));
    assert_eq!(total_voting_power.height, app.block_info().height);

    // Update ADDR1's weight to 2
    let msg = cw4_group::msg::ExecuteMsg::UpdateMembers {
        remove: vec![],
        add: vec![cw4::Member {
            addr: ADDR1.to_string(),
            weight: 2,
        }],
    };

    // Should still be one as voting power should not update until
    // the following block.
    let addr1_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::VotingPowerAtHeight {
                address: ADDR1.to_string(),
                height: None,
            },
        )
        .unwrap();
    assert_eq!(addr1_voting_power.power, Uint128::new(1u128));

    // Same should be true about the groups contract.
    let cw4_power: cw4::MemberResponse = app
        .wrap()
        .query_wasm_smart(
            cw4_addr.clone(),
            &cw4::Cw4QueryMsg::Member {
                addr: ADDR1.to_string(),
                at_height: None,
            },
        )
        .unwrap();
    assert_eq!(cw4_power.weight.unwrap(), 1);

    app.execute_contract(Addr::unchecked(DAO_ADDR), cw4_addr.clone(), &msg, &[])
        .unwrap();
    app.update_block(next_block);

    // Should now be 2
    let addr1_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::VotingPowerAtHeight {
                address: ADDR1.to_string(),
                height: None,
            },
        )
        .unwrap();
    assert_eq!(addr1_voting_power.power, Uint128::new(2u128));
    assert_eq!(addr1_voting_power.height, app.block_info().height);

    // Check we can still get the 1 weight he had last block
    let addr1_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::VotingPowerAtHeight {
                address: ADDR1.to_string(),
                height: Some(app.block_info().height - 1),
            },
        )
        .unwrap();
    assert_eq!(addr1_voting_power.power, Uint128::new(1u128));
    assert_eq!(addr1_voting_power.height, app.block_info().height - 1);

    // Check total power is now 4
    let total_voting_power: TotalPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::TotalPowerAtHeight { height: None },
        )
        .unwrap();
    assert_eq!(total_voting_power.power, Uint128::new(4u128));
    assert_eq!(total_voting_power.height, app.block_info().height);

    // Check total power for last block is 3
    let total_voting_power: TotalPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::TotalPowerAtHeight {
                height: Some(app.block_info().height - 1),
            },
        )
        .unwrap();
    assert_eq!(total_voting_power.power, Uint128::new(3u128));
    assert_eq!(total_voting_power.height, app.block_info().height - 1);

    // Update ADDR1's weight back to 1
    let msg = cw4_group::msg::ExecuteMsg::UpdateMembers {
        remove: vec![],
        add: vec![cw4::Member {
            addr: ADDR1.to_string(),
            weight: 1,
        }],
    };

    app.execute_contract(Addr::unchecked(DAO_ADDR), cw4_addr.clone(), &msg, &[])
        .unwrap();
    app.update_block(next_block);

    // Should now be 1 again
    let addr1_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::VotingPowerAtHeight {
                address: ADDR1.to_string(),
                height: None,
            },
        )
        .unwrap();
    assert_eq!(addr1_voting_power.power, Uint128::new(1u128));
    assert_eq!(addr1_voting_power.height, app.block_info().height);

    // Check total power for current block is now 3
    let total_voting_power: TotalPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::TotalPowerAtHeight { height: None },
        )
        .unwrap();
    assert_eq!(total_voting_power.power, Uint128::new(3u128));
    assert_eq!(total_voting_power.height, app.block_info().height);

    // Check total power for last block is 4
    let total_voting_power: TotalPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::TotalPowerAtHeight {
                height: Some(app.block_info().height - 1),
            },
        )
        .unwrap();
    assert_eq!(total_voting_power.power, Uint128::new(4u128));
    assert_eq!(total_voting_power.height, app.block_info().height - 1);

    // Remove address 2 completely
    let msg = cw4_group::msg::ExecuteMsg::UpdateMembers {
        remove: vec![ADDR2.to_string()],
        add: vec![],
    };

    app.execute_contract(Addr::unchecked(DAO_ADDR), cw4_addr.clone(), &msg, &[])
        .unwrap();
    app.update_block(next_block);

    // ADDR2 power is now 0
    let addr2_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::VotingPowerAtHeight {
                address: ADDR2.to_string(),
                height: None,
            },
        )
        .unwrap();
    assert_eq!(addr2_voting_power.power, Uint128::zero());
    assert_eq!(addr2_voting_power.height, app.block_info().height);

    // Check total power for current block is now 2
    let total_voting_power: TotalPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::TotalPowerAtHeight { height: None },
        )
        .unwrap();
    assert_eq!(total_voting_power.power, Uint128::new(2u128));
    assert_eq!(total_voting_power.height, app.block_info().height);

    // Check total power for last block is 3
    let total_voting_power: TotalPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::TotalPowerAtHeight {
                height: Some(app.block_info().height - 1),
            },
        )
        .unwrap();
    assert_eq!(total_voting_power.power, Uint128::new(3u128));
    assert_eq!(total_voting_power.height, app.block_info().height - 1);

    // Readd ADDR2 with 10 power
    let msg = cw4_group::msg::ExecuteMsg::UpdateMembers {
        remove: vec![],
        add: vec![cw4::Member {
            addr: ADDR2.to_string(),
            weight: 10,
        }],
    };

    app.execute_contract(Addr::unchecked(DAO_ADDR), cw4_addr, &msg, &[])
        .unwrap();
    app.update_block(next_block);

    // ADDR2 power is now 10
    let addr2_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::VotingPowerAtHeight {
                address: ADDR2.to_string(),
                height: None,
            },
        )
        .unwrap();
    assert_eq!(addr2_voting_power.power, Uint128::new(10u128));
    assert_eq!(addr2_voting_power.height, app.block_info().height);

    // Check total power for current block is now 12
    let total_voting_power: TotalPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::TotalPowerAtHeight { height: None },
        )
        .unwrap();
    assert_eq!(total_voting_power.power, Uint128::new(12u128));
    assert_eq!(total_voting_power.height, app.block_info().height);

    // Check total power for last block is 2
    let total_voting_power: TotalPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr,
            &QueryMsg::TotalPowerAtHeight {
                height: Some(app.block_info().height - 1),
            },
        )
        .unwrap();
    assert_eq!(total_voting_power.power, Uint128::new(2u128));
    assert_eq!(total_voting_power.height, app.block_info().height - 1);
}

#[test]
fn test_migrate() {
    let mut app = App::default();

    let initial_members = vec![
        cw4::Member {
            addr: ADDR1.to_string(),
            weight: 1,
        },
        cw4::Member {
            addr: ADDR2.to_string(),
            weight: 1,
        },
        cw4::Member {
            addr: ADDR3.to_string(),
            weight: 1,
        },
    ];

    // Instantiate with no members, error
    let voting_id = app.store_code(dao_voting_cw4_contract());
    let cw4_id = app.store_code(cw4_group_contract());
    let msg = InstantiateMsg {
        group_contract: GroupContract::New {
            cw4_group_code_id: cw4_id,
            initial_members,
        },
    };
    let voting_addr = app
        .instantiate_contract(
            voting_id,
            Addr::unchecked(DAO_ADDR),
            &msg,
            &[],
            "voting module",
            Some(DAO_ADDR.to_string()),
        )
        .unwrap();

    let power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::VotingPowerAtHeight {
                address: ADDR1.to_string(),
                height: None,
            },
        )
        .unwrap();

    app.execute(
        Addr::unchecked(DAO_ADDR),
        CosmosMsg::Wasm(WasmMsg::Migrate {
            contract_addr: voting_addr.to_string(),
            new_code_id: voting_id,
            msg: to_json_binary(&MigrateMsg {}).unwrap(),
        }),
    )
    .unwrap();

    let new_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr,
            &QueryMsg::VotingPowerAtHeight {
                address: ADDR1.to_string(),
                height: None,
            },
        )
        .unwrap();

    assert_eq!(new_power, power)
}

#[test]
fn test_duplicate_member() {
    let mut app = App::default();
    let _voting_addr = setup_test_case(&mut app);
    let voting_id = app.store_code(dao_voting_cw4_contract());
    let cw4_id = app.store_code(cw4_group_contract());
    // Instantiate with members but have a duplicate
    // Total weight is actually 69 but ADDR3 appears twice.
    let msg = InstantiateMsg {
        group_contract: GroupContract::New {
            cw4_group_code_id: cw4_id,
            initial_members: vec![
                cw4::Member {
                    addr: ADDR3.to_string(), // same address above
                    weight: 19,
                },
                cw4::Member {
                    addr: ADDR1.to_string(),
                    weight: 25,
                },
                cw4::Member {
                    addr: ADDR2.to_string(),
                    weight: 25,
                },
                cw4::Member {
                    addr: ADDR3.to_string(),
                    weight: 19,
                },
            ],
        },
    };
    // Previous versions voting power was 100, due to no dedup.
    // Now we error
    // Bug busted : )
    let _voting_addr = app
        .instantiate_contract(
            voting_id,
            Addr::unchecked(DAO_ADDR),
            &msg,
            &[],
            "voting module",
            None,
        )
        .unwrap_err();
}

#[test]
fn test_zero_voting_power() {
    let mut app = App::default();
    let voting_addr = setup_test_case(&mut app);
    app.update_block(next_block);

    let cw4_addr: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::GroupContract {})
        .unwrap();

    // check that ADDR4 weight is 0
    let addr4_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::VotingPowerAtHeight {
                address: ADDR4.to_string(),
                height: None,
            },
        )
        .unwrap();
    assert_eq!(addr4_voting_power.power, Uint128::new(0));
    assert_eq!(addr4_voting_power.height, app.block_info().height);

    // Update ADDR1's weight to 0
    let msg = cw4_group::msg::ExecuteMsg::UpdateMembers {
        remove: vec![],
        add: vec![cw4::Member {
            addr: ADDR1.to_string(),
            weight: 0,
        }],
    };
    app.execute_contract(Addr::unchecked(DAO_ADDR), cw4_addr, &msg, &[])
        .unwrap();

    // Check ADDR1's power is now 0
    let addr1_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::VotingPowerAtHeight {
                address: ADDR1.to_string(),
                height: None,
            },
        )
        .unwrap();
    assert_eq!(addr1_voting_power.power, Uint128::new(0u128));
    assert_eq!(addr1_voting_power.height, app.block_info().height);

    // Check total power is now 2
    let total_voting_power: TotalPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(voting_addr, &QueryMsg::TotalPowerAtHeight { height: None })
        .unwrap();
    assert_eq!(total_voting_power.power, Uint128::new(2u128));
    assert_eq!(total_voting_power.height, app.block_info().height);
}

#[test]
pub fn test_migrate_update_version() {
    let mut deps = mock_dependencies();
    cw2::set_contract_version(&mut deps.storage, "my-contract", "1.0.0").unwrap();
    migrate(deps.as_mut(), mock_env(), MigrateMsg {}).unwrap();
    let version = cw2::get_contract_version(&deps.storage).unwrap();
    assert_eq!(version.version, CONTRACT_VERSION);
    assert_eq!(version.contract, CONTRACT_NAME);
}
