use cosmwasm_std::{to_json_binary, Addr, Binary};
use cw4::{HooksResponse, Member, MemberListResponse, MemberResponse, TotalWeightResponse};
use cw721::{NftInfoResponse, OwnerOfResponse};
use cw_multi_test::{App, Executor};
use dao_cw721_extensions::roles::{ExecuteExt, MetadataExt, QueryExt};
use dao_testing::contracts::{cw721_roles_contract, dao_voting_cw721_staked_contract};
use dao_voting_cw721_staked::msg::{InstantiateMsg as Cw721StakedInstantiateMsg, NftContract};

use crate::error::RolesContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};

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

    // Create a token for bob
    let msg = ExecuteMsg::Mint {
        token_id: "3".to_string(),
        owner: BOB.to_string(),
        token_uri: Some("ipfs://xyz...".to_string()),
        extension: MetadataExt {
            role: None,
            weight: 1,
        },
    };
    app.execute_contract(Addr::unchecked(DAO), cw721_addr.clone(), &msg, &[])
        .unwrap();

    // Query list of members
    let members_list: MemberListResponse = app
        .wrap()
        .query_wasm_smart(
            cw721_addr.clone(),
            &QueryMsg::Extension {
                msg: QueryExt::ListMembers {
                    start_after: None,
                    limit: None,
                },
            },
        )
        .unwrap();
    assert_eq!(
        members_list,
        MemberListResponse {
            members: vec![
                Member {
                    addr: ALICE.to_string(),
                    weight: 2
                },
                Member {
                    addr: BOB.to_string(),
                    weight: 1
                }
            ]
        }
    );

    // Member query returns total weight for alice
    let member: MemberResponse = query_member(&app, &cw721_addr, ALICE, None).unwrap();
    assert_eq!(member.weight, Some(2));

    // Total weight is now 3
    let total: TotalWeightResponse = query_total_weight(&app, &cw721_addr, None).unwrap();
    assert_eq!(total.weight, 3);

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
fn test_minting_and_transfer_permissions() {
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

#[test]
fn test_send_permissions() {
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
    // DAO can mint successfully as the minter
    app.execute_contract(Addr::unchecked(DAO), cw721_addr.clone(), &msg, &[])
        .unwrap();

    // Instantiate an NFT staking voting contract for testing SendNft
    let dao_voting_cw721_staked_id = app.store_code(dao_voting_cw721_staked_contract());
    let cw721_staked_addr = app
        .instantiate_contract(
            dao_voting_cw721_staked_id,
            Addr::unchecked(DAO),
            &Cw721StakedInstantiateMsg {
                nft_contract: NftContract::Existing {
                    address: cw721_addr.to_string(),
                },
                unstaking_duration: None,
                active_threshold: None,
            },
            &[],
            "cw721-staking",
            None,
        )
        .unwrap();

    // Non-minter can't send
    let msg = ExecuteMsg::SendNft {
        contract: cw721_staked_addr.to_string(),
        token_id: "1".to_string(),
        msg: to_json_binary(&Binary::default()).unwrap(),
    };
    app.execute_contract(Addr::unchecked(ALICE), cw721_addr.clone(), &msg, &[])
        .unwrap_err();

    // DAO can send
    app.execute_contract(Addr::unchecked(DAO), cw721_addr.clone(), &msg, &[])
        .unwrap();

    // Staking contract now owns the NFT
    let owner: OwnerOfResponse = query_nft_owner(&app, &cw721_addr, "1").unwrap();
    assert_eq!(owner.owner, cw721_staked_addr.as_str());
}

#[test]
fn test_update_token_role() {
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

    let msg = ExecuteMsg::Extension {
        msg: ExecuteExt::UpdateTokenRole {
            token_id: "1".to_string(),
            role: Some("queen".to_string()),
        },
    };

    // Only admin / minter can update role
    app.execute_contract(Addr::unchecked(ALICE), cw721_addr.clone(), &msg, &[])
        .unwrap_err();

    // Update token role
    app.execute_contract(Addr::unchecked(DAO), cw721_addr.clone(), &msg, &[])
        .unwrap();

    // Token was updated successfully
    let info: NftInfoResponse<MetadataExt> = query_token_info(&app, &cw721_addr, "1").unwrap();
    assert_eq!(info.extension.role, Some("queen".to_string()));

    // Can set to None
    app.execute_contract(
        Addr::unchecked(DAO),
        cw721_addr.clone(),
        &ExecuteMsg::Extension {
            msg: ExecuteExt::UpdateTokenRole {
                token_id: "1".to_string(),
                role: None,
            },
        },
        &[],
    )
    .unwrap();
}

#[test]
fn test_update_token_uri() {
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

    let msg = ExecuteMsg::Extension {
        msg: ExecuteExt::UpdateTokenUri {
            token_id: "1".to_string(),
            token_uri: Some("ipfs://abc...".to_string()),
        },
    };

    // Only admin / minter can update token_uri
    app.execute_contract(Addr::unchecked(ALICE), cw721_addr.clone(), &msg, &[])
        .unwrap_err();

    // Update token_uri
    app.execute_contract(Addr::unchecked(DAO), cw721_addr.clone(), &msg, &[])
        .unwrap();

    // Token was updated successfully
    let info: NftInfoResponse<MetadataExt> = query_token_info(&app, &cw721_addr, "1").unwrap();
    assert_eq!(info.token_uri, Some("ipfs://abc...".to_string()));
}

#[test]
fn test_update_token_weight() {
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

    let msg = ExecuteMsg::Extension {
        msg: ExecuteExt::UpdateTokenWeight {
            token_id: "1".to_string(),
            weight: 2,
        },
    };

    // Only admin / minter can update token weight
    app.execute_contract(Addr::unchecked(ALICE), cw721_addr.clone(), &msg, &[])
        .unwrap_err();

    // Update token weight
    app.execute_contract(Addr::unchecked(DAO), cw721_addr.clone(), &msg, &[])
        .unwrap();

    // Token was updated successfully
    let info: NftInfoResponse<MetadataExt> = query_token_info(&app, &cw721_addr, "1").unwrap();
    assert_eq!(info.extension.weight, 2);

    // New value should be reflected in member's voting weight
    let member: MemberResponse = query_member(&app, &cw721_addr, ALICE, None).unwrap();
    assert_eq!(member.weight, Some(2));

    // Update weight to a smaller value
    app.execute_contract(
        Addr::unchecked(DAO),
        cw721_addr.clone(),
        &ExecuteMsg::Extension {
            msg: ExecuteExt::UpdateTokenWeight {
                token_id: "1".to_string(),
                weight: 1,
            },
        },
        &[],
    )
    .unwrap();

    // New value should be reflected in member's voting weight
    let member: MemberResponse = query_member(&app, &cw721_addr, ALICE, None).unwrap();
    assert_eq!(member.weight, Some(1));

    // Create another token for alice to give her even more total weight
    let msg = ExecuteMsg::Mint {
        token_id: "2".to_string(),
        owner: ALICE.to_string(),
        token_uri: Some("ipfs://xyz...".to_string()),
        extension: MetadataExt {
            role: None,
            weight: 10,
        },
    };
    app.execute_contract(Addr::unchecked(DAO), cw721_addr.clone(), &msg, &[])
        .unwrap();

    // Alice's weight should be updated to include both tokens
    let member: MemberResponse = query_member(&app, &cw721_addr, ALICE, None).unwrap();
    assert_eq!(member.weight, Some(11));

    // Update Alice's second token to 0 weight
    // Update weight to a smaller value
    app.execute_contract(
        Addr::unchecked(DAO),
        cw721_addr.clone(),
        &ExecuteMsg::Extension {
            msg: ExecuteExt::UpdateTokenWeight {
                token_id: "2".to_string(),
                weight: 0,
            },
        },
        &[],
    )
    .unwrap();

    // Alice's voting value should be 1
    let member: MemberResponse = query_member(&app, &cw721_addr, ALICE, None).unwrap();
    assert_eq!(member.weight, Some(1));
}

#[test]
fn test_zero_weight_token() {
    let (mut app, cw721_addr) = setup();

    // Mint token with zero weight
    let msg = ExecuteMsg::Mint {
        token_id: "1".to_string(),
        owner: ALICE.to_string(),
        token_uri: Some("ipfs://xyz...".to_string()),
        extension: MetadataExt {
            role: None,
            weight: 0,
        },
    };
    app.execute_contract(Addr::unchecked(DAO), cw721_addr.clone(), &msg, &[])
        .unwrap();

    // Token was created successfully
    let info: NftInfoResponse<MetadataExt> = query_token_info(&app, &cw721_addr, "1").unwrap();
    assert_eq!(info.extension.weight, 0);

    // Member query returns total weight for alice
    let member: MemberResponse = query_member(&app, &cw721_addr, ALICE, None).unwrap();
    assert_eq!(member.weight, Some(0));
}

#[test]
fn test_hooks() {
    let (mut app, cw721_addr) = setup();

    // Mint initial NFT
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

    let msg = ExecuteMsg::Extension {
        msg: ExecuteExt::AddHook {
            addr: DAO.to_string(),
        },
    };

    // Hook can't be added by non-minter
    app.execute_contract(Addr::unchecked(ALICE), cw721_addr.clone(), &msg, &[])
        .unwrap_err();

    // Hook can be added by the owner / minter
    app.execute_contract(Addr::unchecked(DAO), cw721_addr.clone(), &msg, &[])
        .unwrap();

    // Query hooks
    let hooks: HooksResponse = app
        .wrap()
        .query_wasm_smart(
            cw721_addr.clone(),
            &QueryMsg::Extension {
                msg: QueryExt::Hooks {},
            },
        )
        .unwrap();
    assert_eq!(
        hooks,
        HooksResponse {
            hooks: vec![DAO.to_string()]
        }
    );

    // Test hook fires when a new member is added
    let msg = ExecuteMsg::Mint {
        token_id: "2".to_string(),
        owner: ALICE.to_string(),
        token_uri: Some("ipfs://xyz...".to_string()),
        extension: MetadataExt {
            role: None,
            weight: 1,
        },
    };
    // Should error as the DAO is not a contract, meaning hooks fired
    app.execute_contract(Addr::unchecked(DAO), cw721_addr.clone(), &msg, &[])
        .unwrap_err();

    // Should also error for burn, as this also fires hooks
    let msg = ExecuteMsg::Burn {
        token_id: "1".to_string(),
    };
    app.execute_contract(Addr::unchecked(DAO), cw721_addr.clone(), &msg, &[])
        .unwrap_err();

    let msg = ExecuteMsg::Extension {
        msg: ExecuteExt::RemoveHook {
            addr: DAO.to_string(),
        },
    };

    // Hook can't be removed by non-minter
    app.execute_contract(Addr::unchecked(ALICE), cw721_addr.clone(), &msg, &[])
        .unwrap_err();

    // Hook can be removed by the owner / minter
    app.execute_contract(Addr::unchecked(DAO), cw721_addr.clone(), &msg, &[])
        .unwrap();

    // Minting should now work again as there are no hooks to dead
    app.execute_contract(
        Addr::unchecked(DAO),
        cw721_addr,
        &ExecuteMsg::Mint {
            token_id: "2".to_string(),
            owner: ALICE.to_string(),
            token_uri: Some("ipfs://xyz...".to_string()),
            extension: MetadataExt {
                role: None,
                weight: 1,
            },
        },
        &[],
    )
    .unwrap();
}
