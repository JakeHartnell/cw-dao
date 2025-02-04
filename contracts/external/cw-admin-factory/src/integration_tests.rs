use bech32::{decode, encode, FromBase32, ToBase32, Variant};
use cosmwasm_std::{
    instantiate2_address, to_json_binary, Addr, Binary, CanonicalAddr, Coin, Decimal,
};
use cw_utils::Duration;
use dao_interface::state::{Admin, ModuleInstantiateInfo};
use dao_testing::test_tube::{
    cw4_group::Cw4Group, cw_admin_factory::CwAdminFactory, dao_dao_core::DaoCore,
    dao_proposal_single::DaoProposalSingle, dao_voting_cw4::DaoVotingCw4,
};
use dao_voting::{
    pre_propose::PreProposeInfo,
    threshold::{PercentageThreshold, Threshold},
};
use osmosis_test_tube::{
    osmosis_std::types::cosmwasm::wasm::v1::{
        MsgExecuteContractResponse, QueryCodeRequest, QueryCodeResponse, QueryContractInfoRequest,
        QueryContractInfoResponse,
    },
    Account, ExecuteResponse, OsmosisTestApp, Runner, RunnerError,
};

use cw_admin_factory::msg::ExecuteMsg;

use crate::ContractError;

#[test]
fn test_set_self_admin_instantiate2() {
    let app = OsmosisTestApp::new();
    let accounts = app
        .init_accounts(&[Coin::new(1000000000000000u128, "uosmo")], 10)
        .unwrap();

    // get bech32 prefix from created account
    let prefix = decode(&accounts[0].address()).unwrap().0;

    let cw_admin_factory = CwAdminFactory::new(&app, None, &accounts[0], &[]).unwrap();
    let dao_dao_core_id = DaoCore::upload(&app, &accounts[0]).unwrap();
    let cw4_group_id = Cw4Group::upload(&app, &accounts[0]).unwrap();
    let dao_voting_cw4_id = DaoVotingCw4::upload(&app, &accounts[0]).unwrap();
    let proposal_single_id = DaoProposalSingle::upload(&app, &accounts[0]).unwrap();

    // Get DAO core checksum
    let dao_core_checksum = app
        .query::<QueryCodeRequest, QueryCodeResponse>(
            "/cosmwasm.wasm.v1.Query/Code",
            &QueryCodeRequest {
                code_id: dao_dao_core_id,
            },
        )
        .unwrap()
        .code_info
        .unwrap()
        .data_hash;

    let msg = dao_interface::msg::InstantiateMsg {
        dao_uri: None,
        admin: None,
        name: "DAO DAO".to_string(),
        description: "A DAO that makes DAO tooling".to_string(),
        image_url: None,
        automatically_add_cw20s: false,
        automatically_add_cw721s: false,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: dao_voting_cw4_id,
            msg: to_json_binary(&dao_voting_cw4::msg::InstantiateMsg {
                group_contract: dao_voting_cw4::msg::GroupContract::New {
                    cw4_group_code_id: cw4_group_id,
                    initial_members: vec![cw4::Member {
                        addr: accounts[0].address(),
                        weight: 1,
                    }],
                },
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

    let salt = Binary::from("salt".as_bytes());
    let res: ExecuteResponse<MsgExecuteContractResponse> = cw_admin_factory
        .execute(
            &ExecuteMsg::Instantiate2ContractWithSelfAdmin {
                instantiate_msg: to_json_binary(&msg).unwrap(),
                code_id: dao_dao_core_id,
                label: "first".to_string(),
                salt: salt.clone(),
                expect: None,
            },
            &[],
            &accounts[0],
        )
        .unwrap();
    let core_addr = &res
        .events
        .iter()
        .find(|e| {
            e.ty == "instantiate"
                && e.attributes
                    .iter()
                    .any(|a| a.key == "code_id" && a.value == dao_dao_core_id.to_string())
        })
        .unwrap()
        .attributes
        .iter()
        .find(|a| a.key == "_contract_address")
        .unwrap()
        .value;

    // Check that admin of core address is itself
    let core_admin = app
        .query::<QueryContractInfoRequest, QueryContractInfoResponse>(
            "/cosmwasm.wasm.v1.Query/ContractInfo",
            &QueryContractInfoRequest {
                address: core_addr.to_string(),
            },
        )
        .unwrap()
        .contract_info
        .unwrap()
        .admin;
    assert_eq!(&core_admin, core_addr);

    // Check that the address matches the predicted address
    let canonical_factory = addr_canonicalize(&prefix, &cw_admin_factory.contract_addr);
    let expected_addr = addr_humanize(
        &prefix,
        &instantiate2_address(&dao_core_checksum, &canonical_factory, salt.as_slice()).unwrap(),
    );
    assert_eq!(core_addr, expected_addr.as_str());

    // Check that it succeeds when expect matches.
    let salt = Binary::from("salt_two".as_bytes());
    let expected_addr = addr_humanize(
        &prefix,
        &instantiate2_address(&dao_core_checksum, &canonical_factory, salt.as_slice()).unwrap(),
    );
    let res: ExecuteResponse<MsgExecuteContractResponse> = cw_admin_factory
        .execute(
            &ExecuteMsg::Instantiate2ContractWithSelfAdmin {
                instantiate_msg: to_json_binary(&msg).unwrap(),
                code_id: dao_dao_core_id,
                label: "second".to_string(),
                salt: salt.clone(),
                expect: Some(expected_addr.to_string()),
            },
            &[],
            &accounts[0],
        )
        .unwrap();
    let core_addr = &res
        .events
        .iter()
        .find(|e| {
            e.ty == "instantiate"
                && e.attributes
                    .iter()
                    .any(|a| a.key == "code_id" && a.value == dao_dao_core_id.to_string())
        })
        .unwrap()
        .attributes
        .iter()
        .find(|a| a.key == "_contract_address")
        .unwrap()
        .value;
    assert_eq!(core_addr, expected_addr.as_str());

    // Check that admin of core address is itself
    let core_admin = app
        .query::<QueryContractInfoRequest, QueryContractInfoResponse>(
            "/cosmwasm.wasm.v1.Query/ContractInfo",
            &QueryContractInfoRequest {
                address: core_addr.clone(),
            },
        )
        .unwrap()
        .contract_info
        .unwrap()
        .admin;
    assert_eq!(&core_admin, core_addr);

    // Check that it fails when expect does not match.
    let salt = Binary::from("salt_mismatch".as_bytes());
    let actual_addr = addr_humanize(
        &prefix,
        &instantiate2_address(&dao_core_checksum, &canonical_factory, salt.as_slice()).unwrap(),
    );
    let err = cw_admin_factory
        .execute(
            &ExecuteMsg::Instantiate2ContractWithSelfAdmin {
                instantiate_msg: to_json_binary(&msg).unwrap(),
                code_id: dao_dao_core_id,
                label: "third".to_string(),
                salt: salt.clone(),
                expect: Some(cw_admin_factory.contract_addr.clone()),
            },
            &[],
            &accounts[0],
        )
        .unwrap_err();
    assert_eq!(
        err,
        RunnerError::ExecuteError {
            msg: format!(
                "failed to execute message; message index: 0: dispatch: submessages: reply: {}: execute wasm contract failed",
                ContractError::UnexpectedContractAddress {
                    expected: cw_admin_factory.contract_addr.clone(),
                    actual: actual_addr.to_string(),
                }
            )
        },
    );
}

fn addr_canonicalize(prefix: &str, input: &str) -> CanonicalAddr {
    let (p, decoded, variant) = decode(input).unwrap();
    if p == prefix && variant == Variant::Bech32 {
        return Vec::<u8>::from_base32(&decoded).unwrap().into();
    }
    panic!("Invalid address: {}", input);
}

fn addr_humanize(prefix: &str, canonical: &CanonicalAddr) -> Addr {
    let encoded = encode(prefix, canonical.as_slice().to_base32(), Variant::Bech32).unwrap();
    Addr::unchecked(encoded)
}
