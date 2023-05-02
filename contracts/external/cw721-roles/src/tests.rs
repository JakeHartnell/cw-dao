use cosmwasm_std::Addr;
use cw4::{MemberResponse, TotalWeightResponse};
use cw721::{NftInfoResponse, OwnerOfResponse};
use cw721_base::InstantiateMsg;
use cw_multi_test::{App, Executor};
use dao_testing::contracts::cw721_roles_contract;

use crate::error::RolesContractError;
use crate::msg::{ExecuteMsg, MetadataExt, QueryExt, QueryMsg};

const ALICE: &str = "alice";
const BOB: &str = "bob";
const DAO: &str = "dao";

pub fn setup() -> (App, Addr) {
    let mut app = App::default();

    let cw721_id = app.store_code(cw721_roles_contract());
    let cw721_addr = app
        .instantiate_contract(
            cw721_id,
            Addr::unchecked(DAO),
            &InstantiateMsg {
                name: "bad kids".to_string(),
                symbol: "bad kids".to_string(),
                minter: DAO.to_string(),
            },
            &[],
            "cw721_roles".to_string(),
            None,
        )
        .unwrap();

    (app, cw721_addr)
}

pub fn query_nft_owner(
    app: &App,
    nft: &Addr,
    token_id: &str,
) -> Result<cw721::OwnerOfResponse, RolesContractError> {
    let owner = app.wrap().query_wasm_smart(
        nft,
        &QueryMsg::OwnerOf {
            token_id: token_id.to_string(),
            include_expired: None,
        },
    )?;
    Ok(owner)
}

pub fn query_member(
    app: &App,
    nft: &Addr,
    member: &str,
    at_height: Option<u64>,
) -> Result<MemberResponse, RolesContractError> {
    let member = app.wrap().query_wasm_smart(
        nft,
        &QueryMsg::Extension {
            msg: QueryExt::Member {
                addr: member.to_string(),
                at_height,
            },
        },
    )?;
    Ok(member)
}

pub fn query_total_weight(
    app: &App,
    nft: &Addr,
    at_height: Option<u64>,
) -> Result<TotalWeightResponse, RolesContractError> {
    let member = app.wrap().query_wasm_smart(
        nft,
        &QueryMsg::Extension {
            msg: QueryExt::TotalWeight { at_height },
        },
    )?;
    Ok(member)
}

pub fn query_token_info(
    app: &App,
    nft: &Addr,
    token_id: &str,
) -> Result<NftInfoResponse<MetadataExt>, RolesContractError> {
    let info = app.wrap().query_wasm_smart(
        nft,
        &QueryMsg::NftInfo {
            token_id: token_id.to_string(),
        },
    )?;
    Ok(info)
}

#[test]
fn test_minting_and_burning() {
    let (mut app, cw721_addr) = setup();

    // Mint token
    let msg = ExecuteMsg::Mint {
        token_id: "1".to_string(),
        owner: ALICE.to_string(),
        token_uri: Some("ipfs://xyz...".to_string()),
        extension: MetadataExt {
            role: None,
            weight: 1,
        },
    };
    app.execute_contract(Addr::unchecked(DAO), cw721_addr.clone(), &msg, &[])
        .unwrap();

    // Token was created successfully
    let info: NftInfoResponse<MetadataExt> = query_token_info(&app, &cw721_addr, "1").unwrap();
    assert_eq!(info.extension.weight, 1);

    // Create another token for alice to give her even more total weight
    let msg = ExecuteMsg::Mint {
        token_id: "2".to_string(),
        owner: ALICE.to_string(),
        token_uri: Some("ipfs://xyz...".to_string()),
        extension: MetadataExt {
            role: None,
            weight: 1,
        },
    };
    app.execute_contract(Addr::unchecked(DAO), cw721_addr.clone(), &msg, &[])
        .unwrap();

    // Member query returns total weight for alice
    let member: MemberResponse = query_member(&app, &cw721_addr, ALICE, None).unwrap();
    assert_eq!(member.weight, Some(2));

    // Total weight is now 2
    let total: TotalWeightResponse = query_total_weight(&app, &cw721_addr, None).unwrap();
    assert_eq!(total.weight, 2);

    // Burn a role for alice
    let msg = ExecuteMsg::Burn {
        token_id: "2".to_string(),
    };
    app.execute_contract(Addr::unchecked(DAO), cw721_addr.clone(), &msg, &[])
        .unwrap();

    // Token is now gone
    let res = query_token_info(&app, &cw721_addr, "2");
    assert!(res.is_err());

    // Alice's weight has been update acordingly
    let member: MemberResponse = query_member(&app, &cw721_addr, ALICE, None).unwrap();
    assert_eq!(member.weight, Some(1));
}

#[test]
fn test_permissions() {
    let (mut app, cw721_addr) = setup();

    // Mint token
    let msg = ExecuteMsg::Mint {
        token_id: "1".to_string(),
        owner: ALICE.to_string(),
        token_uri: Some("ipfs://xyz...".to_string()),
        extension: MetadataExt {
            role: Some("member".to_string()),
            weight: 1,
        },
    };

    // Non-minter can't mint
    app.execute_contract(Addr::unchecked(ALICE), cw721_addr.clone(), &msg, &[])
        .unwrap_err();

    // DAO can mint successfully as the minter
    app.execute_contract(Addr::unchecked(DAO), cw721_addr.clone(), &msg, &[])
        .unwrap();

    // Non-minter can't transfer
    let msg = ExecuteMsg::TransferNft {
        recipient: BOB.to_string(),
        token_id: "1".to_string(),
    };
    app.execute_contract(Addr::unchecked(ALICE), cw721_addr.clone(), &msg, &[])
        .unwrap_err();

    // DAO can transfer
    app.execute_contract(Addr::unchecked(DAO), cw721_addr.clone(), &msg, &[])
        .unwrap();

    let owner: OwnerOfResponse = query_nft_owner(&app, &cw721_addr, "1").unwrap();
    assert_eq!(owner.owner, BOB);
}
