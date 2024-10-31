use cosmwasm_std::storage_keys::to_length_prefixed_nested;
use cosmwasm_std::testing::{mock_dependencies, mock_env};
use cosmwasm_std::{to_json_vec, Addr, Decimal, Storage, Uint128};
use cw721_controllers::{NftClaim, NftClaimsResponse};
use cw_multi_test::{next_block, Executor};
use cw_storage_plus::Map;
use cw_utils::Duration;
use dao_interface::voting::IsActiveResponse;
use dao_voting::threshold::{ActiveThreshold, ActiveThresholdResponse};

use crate::msg::OnftCollection;
use crate::testing::execute::{
    cancel_stake, claim_legacy_nfts, claim_specific_nfts, confirm_stake_nft, prepare_stake_nft,
    send_nft,
};
use crate::testing::queries::query_dao;
use crate::testing::DAO;
use crate::{
    contract::{migrate, CONTRACT_NAME, CONTRACT_VERSION},
    msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg},
    testing::{
        execute::{
            claim_nfts, mint_and_stake_nft, mint_nft, stake_nft, unstake_nfts, update_config,
        },
        queries::{query_config, query_hooks, query_nft_owner, query_total_and_voting_power},
    },
};

use super::{
    execute::{add_hook, remove_hook},
    is_error,
    queries::{query_claims, query_info, query_staked_nfts, query_total_power, query_voting_power},
    setup_test, CommonTest, STAKER,
};

// I can stake tokens, voting power and total power is updated one
// block later.
#[test]
fn test_stake_tokens() -> anyhow::Result<()> {
    let CommonTest {
        mut app,
        module,
        nft,
        ..
    } = setup_test(None, None);

    let total_power = query_total_power(&app, &module, None)?;
    let voting_power = query_voting_power(&app, &module, STAKER, None)?;

    assert_eq!(total_power.power, Uint128::zero());
    assert_eq!(total_power.height, app.block_info().height);

    assert_eq!(voting_power.power, Uint128::zero());
    assert_eq!(voting_power.height, app.block_info().height);

    mint_and_stake_nft(&mut app, &nft, &module, STAKER, "1")?;

    // Voting powers are not updated until a block has passed.
    let (total, personal) = query_total_and_voting_power(&app, &module, STAKER, None)?;
    assert!(total.is_zero());
    assert!(personal.is_zero());

    app.update_block(next_block);

    let (total, personal) = query_total_and_voting_power(&app, &module, STAKER, None)?;
    assert_eq!(total, Uint128::new(1));
    assert_eq!(personal, Uint128::new(1));

    // Registering duplicate token IDs does not provide more voting power.
    mint_nft(&mut app, &nft, STAKER, "2")?;
    prepare_stake_nft(&mut app, &module, STAKER, "2")?;
    send_nft(&mut app, &nft, "2", STAKER, module.as_str())?;
    app.execute_contract(
        Addr::unchecked(STAKER),
        module.clone(),
        &ExecuteMsg::ConfirmStake {
            token_ids: vec!["2".to_string(), "2".to_string(), "2".to_string()],
        },
        &[],
    )?;

    app.update_block(next_block);

    let (total, personal) = query_total_and_voting_power(&app, &module, STAKER, None)?;
    assert_eq!(total, Uint128::new(2));
    assert_eq!(personal, Uint128::new(2));

    Ok(())
}

// I can unstake tokens. Unstaking more than one token at once
// works. I can not unstake a token more than once. I can not unstake
// another addresses' token. Voting power and total power is updated
// when I unstake.
#[test]
fn test_unstake_tokens_no_claims() -> anyhow::Result<()> {
    let CommonTest {
        mut app,
        module,
        nft,
        ..
    } = setup_test(None, None);

    let friend = "friend";

    mint_and_stake_nft(&mut app, &nft, &module, STAKER, "1")?;
    mint_and_stake_nft(&mut app, &nft, &module, STAKER, "2")?;
    mint_and_stake_nft(&mut app, &nft, &module, STAKER, "3")?;

    mint_nft(&mut app, &nft, friend, "4")?;
    mint_nft(&mut app, &nft, friend, "5")?;
    stake_nft(&mut app, &nft, &module, friend, "4")?;
    stake_nft(&mut app, &nft, &module, friend, "5")?;

    app.update_block(next_block);

    let (total, personal) = query_total_and_voting_power(&app, &module, STAKER, None)?;
    assert_eq!(total, Uint128::new(5));
    assert_eq!(personal, Uint128::new(3));

    unstake_nfts(&mut app, &module, STAKER, &["1", "2"])?;

    // Voting power is updated when I unstake. Waits a block as it's a
    // snapshot map.
    let (total, personal) = query_total_and_voting_power(&app, &module, STAKER, None)?;
    assert_eq!(total, Uint128::new(5));
    assert_eq!(personal, Uint128::new(3));
    app.update_block(next_block);
    let (total, personal) = query_total_and_voting_power(&app, &module, STAKER, None)?;
    assert_eq!(total, Uint128::new(3));
    assert_eq!(personal, Uint128::new(1));

    // I can not unstake tokens I do not own. Anyhow can't figure out
    // how to downcast this error so we check for the expected string.
    let res = unstake_nfts(&mut app, &module, STAKER, &["4"]);
    is_error!(res => "Can not unstake that which you have not staked (unstaking 4)");

    let res = unstake_nfts(&mut app, &module, STAKER, &["5", "4"]);
    is_error!(res => "Can not unstake that which you have not staked (unstaking 5)");

    let res = unstake_nfts(&mut app, &module, STAKER, &["☯️", "4"]);
    is_error!(res => "Can not unstake that which you have not staked (unstaking ☯️)");

    // I can not unstake tokens more than once.
    let res = unstake_nfts(&mut app, &module, STAKER, &["1"]);
    is_error!(res => "Can not unstake that which you have not staked (unstaking 1)");

    Ok(())
}

// I cannot unstake zero tokens.
#[test]
fn test_unstake_zero_tokens() -> anyhow::Result<()> {
    let CommonTest {
        mut app, module, ..
    } = setup_test(None, None);

    let res = unstake_nfts(&mut app, &module, STAKER, &[]);
    is_error!(res => "Can't unstake zero NFTs.");

    Ok(())
}

// I can update the unstaking duration and the owner. Only the owner
// may do this. I can unset the owner. Updating the unstaking duration
// does not impact outstanding claims.
#[test]
fn test_update_config() -> anyhow::Result<()> {
    let CommonTest {
        mut app,
        module,
        nft,
        ..
    } = setup_test(Some(Duration::Height(3)), None);

    // non-DAO cannot update config
    let res = update_config(&mut app, &module, STAKER, Some(Duration::Time(1)));
    is_error!(res => "Unauthorized");

    mint_and_stake_nft(&mut app, &nft, &module, STAKER, "1")?;
    mint_and_stake_nft(&mut app, &nft, &module, STAKER, "2")?;

    unstake_nfts(&mut app, &module, STAKER, &["1"])?;

    let claims = query_claims(&app, &module, STAKER)?;
    assert_eq!(
        claims,
        NftClaimsResponse {
            nft_claims: vec![NftClaim {
                token_id: "1".to_string(),
                release_at: cw_utils::Expiration::AtHeight(app.block_info().height + 3)
            }]
        }
    );

    // Update config to invalid duration fails
    let err = update_config(&mut app, &module, DAO, Some(Duration::Time(0))).unwrap_err();
    assert_eq!(
        err.root_cause().to_string(),
        "Invalid unstaking duration, unstaking duration cannot be 0".to_string()
    );

    // Update duration
    update_config(&mut app, &module, DAO, Some(Duration::Time(1)))?;

    // Existing claims should remain unchanged.
    let claims = query_claims(&app, &module, STAKER)?;
    assert_eq!(
        claims,
        NftClaimsResponse {
            nft_claims: vec![NftClaim {
                token_id: "1".to_string(),
                release_at: cw_utils::Expiration::AtHeight(app.block_info().height + 3)
            }]
        }
    );

    // New claims should reflect the new unstaking duration. Old ones
    // should not.
    unstake_nfts(&mut app, &module, STAKER, &["2"])?;
    let claims = query_claims(&app, &module, STAKER)?;
    assert_eq!(
        claims,
        NftClaimsResponse {
            nft_claims: vec![
                NftClaim {
                    token_id: "1".to_string(),
                    release_at: cw_utils::Expiration::AtHeight(app.block_info().height + 3)
                },
                NftClaim {
                    token_id: "2".to_string(),
                    release_at: Duration::Time(1).after(&app.block_info())
                }
            ]
        }
    );

    let info = app.block_info();
    app.update_block(|block| {
        block.height += 3;
        block.time = match Duration::Time(1).after(&info) {
            cw_utils::Expiration::AtTime(timestamp) => timestamp,
            _ => panic!("there should really be an easier way to do this"),
        }
    });

    // Do a claim for good measure.
    claim_nfts(&mut app, &module, STAKER)?;
    let claims = query_claims(&app, &module, STAKER)?;
    assert_eq!(claims, NftClaimsResponse { nft_claims: vec![] });

    Ok(())
}

// I can query my pending claims. Attempting to claim with nothing to
// claim results in an error. Attempting to claim with tokens to claim
// results in me owning those tokens.
#[test]
fn test_claims() -> anyhow::Result<()> {
    let CommonTest {
        mut app,
        module,
        nft,
        ..
    } = setup_test(Some(Duration::Height(1)), None);

    mint_and_stake_nft(&mut app, &nft, &module, STAKER, "1")?;
    mint_and_stake_nft(&mut app, &nft, &module, STAKER, "2")?;
    mint_and_stake_nft(&mut app, &nft, &module, STAKER, "3")?;

    let claims = query_claims(&app, &module, STAKER)?;
    assert_eq!(claims.nft_claims, vec![]);

    let res = claim_nfts(&mut app, &module, STAKER);
    is_error!(res => "Nothing to claim");

    unstake_nfts(&mut app, &module, STAKER, &["2"])?;

    let claims = query_claims(&app, &module, STAKER)?;
    assert_eq!(
        claims.nft_claims,
        vec![NftClaim {
            token_id: "2".to_string(),
            release_at: cw_utils::Expiration::AtHeight(app.block_info().height + 1)
        }]
    );

    // Claim now exists, but is not yet expired. Nothing to claim.
    let res = claim_nfts(&mut app, &module, STAKER);
    is_error!(res => "Nothing to claim");

    app.update_block(next_block);
    claim_nfts(&mut app, &module, STAKER)?;

    let owner = query_nft_owner(&app, &nft, "2")?;
    assert_eq!(owner, STAKER.to_string());

    Ok(())
}

// I can query and claim my pending legacy claims and non-legacy claims.
#[test]
pub fn test_legacy_claims_work() -> anyhow::Result<()> {
    let CommonTest {
        mut app,
        module,
        nft,
        ..
    } = setup_test(Some(Duration::Height(1)), None);

    mint_and_stake_nft(&mut app, &nft, &module, STAKER, "1")?;
    mint_and_stake_nft(&mut app, &nft, &module, STAKER, "2")?;
    mint_and_stake_nft(&mut app, &nft, &module, STAKER, "3")?;
    mint_and_stake_nft(&mut app, &nft, &module, STAKER, "4")?;
    mint_and_stake_nft(&mut app, &nft, &module, STAKER, "5")?;

    let claims = query_claims(&app, &module, STAKER)?;
    assert_eq!(claims.nft_claims, vec![]);

    let res = claim_legacy_nfts(&mut app, &module, STAKER);
    is_error!(res => "Nothing to claim");
    let res = claim_nfts(&mut app, &module, STAKER);
    is_error!(res => "Nothing to claim");

    // insert legacy claims manually

    // taken from cw-multi-test's WasmKeeper::contract_storage in wasm.rs
    let mut module_namespace = b"contract_data/".to_vec();
    module_namespace.extend_from_slice(module.as_bytes());
    let prefix = to_length_prefixed_nested(&[b"wasm", &module_namespace]);
    let key = Map::<&Addr, Vec<NftClaim>>::new("nft_claims").key(&Addr::unchecked(STAKER));
    let mut legacy_nft_claims_key = prefix;
    legacy_nft_claims_key.extend_from_slice(&key);

    let block = app.block_info();
    app.storage_mut().set(
        &legacy_nft_claims_key,
        &to_json_vec(&vec![cw721_controllers_v250::NftClaim {
            token_id: "4".to_string(),
            release_at: Duration::Height(1).after(&block),
        }])
        .unwrap(),
    );

    let claims = query_claims(&app, &module, STAKER)?;
    assert_eq!(
        claims.nft_claims,
        vec![NftClaim {
            token_id: "4".to_string(),
            release_at: cw_utils::Expiration::AtHeight(app.block_info().height + 1)
        }]
    );

    // claim now exists, but is not yet expired. Nothing to claim.
    let res = claim_legacy_nfts(&mut app, &module, STAKER);
    is_error!(res => "Nothing to claim");
    let res = claim_nfts(&mut app, &module, STAKER);
    is_error!(res => "Nothing to claim");

    app.update_block(next_block);

    // no non-legacy claims
    let res = claim_nfts(&mut app, &module, STAKER);
    is_error!(res => "Nothing to claim");

    // legacy claim works
    claim_legacy_nfts(&mut app, &module, STAKER).unwrap();
    let owner = query_nft_owner(&app, &nft, "4")?;
    assert_eq!(owner, STAKER.to_string());

    // unstake non-legacy
    unstake_nfts(&mut app, &module, STAKER, &["2"])?;

    let claims = query_claims(&app, &module, STAKER)?;
    assert_eq!(
        claims.nft_claims,
        vec![NftClaim {
            token_id: "2".to_string(),
            release_at: cw_utils::Expiration::AtHeight(app.block_info().height + 1)
        }]
    );

    // Claim now exists, but is not yet expired. Nothing to claim.
    let res = claim_legacy_nfts(&mut app, &module, STAKER);
    is_error!(res => "Nothing to claim");
    let res = claim_nfts(&mut app, &module, STAKER);
    is_error!(res => "Nothing to claim");

    app.update_block(next_block);

    // no legacy claims
    let res = claim_legacy_nfts(&mut app, &module, STAKER);
    is_error!(res => "Nothing to claim");

    claim_nfts(&mut app, &module, STAKER)?;

    let owner = query_nft_owner(&app, &nft, "2")?;
    assert_eq!(owner, STAKER.to_string());

    // unstake another non-legacy
    unstake_nfts(&mut app, &module, STAKER, &["3"])?;

    let claims = query_claims(&app, &module, STAKER)?;
    assert_eq!(
        claims.nft_claims,
        vec![NftClaim {
            token_id: "3".to_string(),
            release_at: cw_utils::Expiration::AtHeight(app.block_info().height + 1)
        }]
    );

    app.update_block(next_block);

    claim_specific_nfts(&mut app, &module, STAKER, &["3".to_string()])?;
    let owner = query_nft_owner(&app, &nft, "3")?;
    assert_eq!(owner, STAKER.to_string());

    // unstake legacy
    let block = app.block_info();
    app.storage_mut().set(
        &legacy_nft_claims_key,
        &to_json_vec(&vec![cw721_controllers_v250::NftClaim {
            token_id: "5".to_string(),
            release_at: Duration::Height(1).after(&block),
        }])
        .unwrap(),
    );
    // unstake non-legacy
    unstake_nfts(&mut app, &module, STAKER, &["1"])?;

    let claims = query_claims(&app, &module, STAKER)?;
    assert_eq!(
        claims.nft_claims,
        vec![
            NftClaim {
                token_id: "5".to_string(),
                release_at: cw_utils::Expiration::AtHeight(app.block_info().height + 1)
            },
            NftClaim {
                token_id: "1".to_string(),
                release_at: cw_utils::Expiration::AtHeight(app.block_info().height + 1)
            }
        ]
    );

    app.update_block(next_block);

    // both claims should be ready to claim
    claim_legacy_nfts(&mut app, &module, STAKER)?;
    claim_nfts(&mut app, &module, STAKER)?;

    let owner = query_nft_owner(&app, &nft, "1")?;
    assert_eq!(owner, STAKER.to_string());
    let owner = query_nft_owner(&app, &nft, "5")?;
    assert_eq!(owner, STAKER.to_string());

    // no claims left
    let claims = query_claims(&app, &module, STAKER)?;
    assert_eq!(claims.nft_claims, vec![]);

    Ok(())
}

// I can list all of the currently staked NFTs for an address.
#[test]
fn test_list_staked_nfts() -> anyhow::Result<()> {
    let CommonTest {
        mut app,
        module,
        nft,
        ..
    } = setup_test(Some(Duration::Height(1)), None);

    mint_and_stake_nft(&mut app, &nft, &module, STAKER, "1")?;
    mint_and_stake_nft(&mut app, &nft, &module, STAKER, "2")?;
    mint_and_stake_nft(&mut app, &nft, &module, STAKER, "3")?;

    let deardrie = "deardrie";
    mint_nft(&mut app, &nft, deardrie, "4")?;
    mint_nft(&mut app, &nft, deardrie, "5")?;

    let nfts = query_staked_nfts(&app, &module, deardrie, None, None)?;
    assert!(nfts.is_empty());

    stake_nft(&mut app, &nft, &module, deardrie, "4")?;
    stake_nft(&mut app, &nft, &module, deardrie, "5")?;

    let nfts = query_staked_nfts(&app, &module, deardrie, None, None)?;
    assert_eq!(nfts, vec!["4".to_string(), "5".to_string()]);

    let nfts = query_staked_nfts(&app, &module, STAKER, Some("1".to_string()), Some(0))?;
    assert!(nfts.is_empty());

    let nfts = query_staked_nfts(&app, &module, STAKER, Some("3".to_string()), None)?;
    assert!(nfts.is_empty());
    let nfts = query_staked_nfts(&app, &module, STAKER, Some("3".to_string()), Some(500))?;
    assert!(nfts.is_empty());

    let nfts = query_staked_nfts(&app, &module, STAKER, Some("1".to_string()), Some(2))?;
    assert_eq!(nfts, vec!["2".to_string(), "3".to_string()]);

    unstake_nfts(&mut app, &module, STAKER, &["2"])?;
    let nfts = query_staked_nfts(&app, &module, STAKER, Some("1".to_string()), Some(2))?;
    assert_eq!(nfts, vec!["3".to_string()]);

    Ok(())
}

#[test]
fn test_info_query_works() -> anyhow::Result<()> {
    let CommonTest { app, module, .. } = setup_test(None, None);
    let info = query_info(&app, &module)?;
    assert_eq!(info.info.version, env!("CARGO_PKG_VERSION").to_string());
    Ok(())
}

#[test]
fn test_dao_query_works() -> anyhow::Result<()> {
    let CommonTest { app, module, .. } = setup_test(None, None);
    let dao = query_dao(&app, &module)?;
    assert_eq!(dao, DAO.to_string());
    Ok(())
}

// The owner may add and remove hooks.
#[test]
fn test_add_remove_hooks() -> anyhow::Result<()> {
    let CommonTest {
        mut app,
        module,
        nft,
        ..
    } = setup_test(None, None);

    add_hook(&mut app, &module, DAO, "meow")?;
    remove_hook(&mut app, &module, DAO, "meow")?;

    // Minting NFT works if no hooks
    mint_and_stake_nft(&mut app, &nft, &module, STAKER, "1").unwrap();

    // Add a hook to a fake contract called "meow"
    add_hook(&mut app, &module, DAO, "meow")?;

    let hooks = query_hooks(&app, &module)?;
    assert_eq!(hooks.hooks, vec!["meow".to_string()]);

    // Minting / staking now doesn't work because meow isn't a contract
    // This failure means the hook is working
    mint_and_stake_nft(&mut app, &nft, &module, STAKER, "1").unwrap_err();

    let res = add_hook(&mut app, &module, DAO, "meow");
    is_error!(res => "Given address already registered as a hook");

    let res = remove_hook(&mut app, &module, DAO, "blue");
    is_error!(res => "Given address not registered as a hook");

    let res = add_hook(&mut app, &module, "ekez", "evil");
    is_error!(res => "Unauthorized");
    let res = remove_hook(&mut app, &module, "ekez", "evil");
    is_error!(res => "Unauthorized");

    Ok(())
}

#[test]
#[should_panic(expected = "Active threshold count must be greater than zero")]
fn test_instantiate_zero_active_threshold_count() {
    setup_test(
        None,
        Some(ActiveThreshold::AbsoluteCount {
            count: Uint128::zero(),
        }),
    );
}

#[test]
#[should_panic(expected = "Absolute count threshold cannot be greater than the total token supply")]
fn test_instantiate_invalid_active_threshold_count() {
    setup_test(
        None,
        Some(ActiveThreshold::AbsoluteCount {
            count: Uint128::new(100),
        }),
    );
}

#[test]
fn test_active_threshold_absolute_count() {
    let CommonTest {
        mut app,
        module_id,
        nft,
        ..
    } = setup_test(None, None);

    mint_nft(&mut app, &nft, STAKER, "1").unwrap();
    mint_nft(&mut app, &nft, STAKER, "2").unwrap();
    mint_nft(&mut app, &nft, STAKER, "3").unwrap();

    let module = app
        .instantiate_contract(
            module_id,
            Addr::unchecked(DAO),
            &InstantiateMsg {
                onft_collection: OnftCollection::Existing {
                    id: nft.to_string(),
                },
                unstaking_duration: None,
                active_threshold: Some(ActiveThreshold::AbsoluteCount {
                    count: Uint128::new(3),
                }),
            },
            &[],
            "onft_voting",
            None,
        )
        .unwrap();

    // Get collection ID
    let onft_collection_id = query_config(&app, &module).unwrap().onft_collection_id;

    // Not active as none staked
    let is_active: IsActiveResponse = app
        .wrap()
        .query_wasm_smart(module.clone(), &QueryMsg::IsActive {})
        .unwrap();
    assert!(!is_active.active);

    // Stake NFTs
    stake_nft(&mut app, &onft_collection_id, &module, STAKER, "1").unwrap();
    stake_nft(&mut app, &onft_collection_id, &module, STAKER, "2").unwrap();
    stake_nft(&mut app, &onft_collection_id, &module, STAKER, "3").unwrap();

    app.update_block(next_block);

    // Active as enough staked
    let is_active: IsActiveResponse = app
        .wrap()
        .query_wasm_smart(module, &QueryMsg::IsActive {})
        .unwrap();
    assert!(is_active.active);
}

#[test]
fn test_active_threshold_percent() {
    let CommonTest {
        mut app,
        module_id,
        nft,
        ..
    } = setup_test(None, None);

    mint_nft(&mut app, &nft, STAKER, "1").unwrap();

    let module = app
        .instantiate_contract(
            module_id,
            Addr::unchecked(DAO),
            &InstantiateMsg {
                onft_collection: OnftCollection::Existing {
                    id: nft.to_string(),
                },
                unstaking_duration: None,
                active_threshold: Some(ActiveThreshold::Percentage {
                    percent: Decimal::percent(20),
                }),
            },
            &[],
            "onft_voting",
            None,
        )
        .unwrap();

    // Get collection ID
    let onft_collection_id = query_config(&app, &module).unwrap().onft_collection_id;

    // Not active as none staked
    let is_active: IsActiveResponse = app
        .wrap()
        .query_wasm_smart(module.clone(), &QueryMsg::IsActive {})
        .unwrap();
    assert!(!is_active.active);

    // Stake NFTs
    stake_nft(&mut app, &onft_collection_id, &module, STAKER, "1").unwrap();
    app.update_block(next_block);

    // Active as enough staked
    let is_active: IsActiveResponse = app
        .wrap()
        .query_wasm_smart(module, &QueryMsg::IsActive {})
        .unwrap();
    assert!(is_active.active);
}

#[test]
fn test_active_threshold_percent_rounds_up() {
    let CommonTest {
        mut app,
        module_id,
        nft,
        ..
    } = setup_test(None, None);

    mint_nft(&mut app, &nft, STAKER, "1").unwrap();
    mint_nft(&mut app, &nft, STAKER, "2").unwrap();
    mint_nft(&mut app, &nft, STAKER, "3").unwrap();
    mint_nft(&mut app, &nft, STAKER, "4").unwrap();
    mint_nft(&mut app, &nft, STAKER, "5").unwrap();

    let module = app
        .instantiate_contract(
            module_id,
            Addr::unchecked(DAO),
            &InstantiateMsg {
                onft_collection: OnftCollection::Existing {
                    id: nft.to_string(),
                },
                unstaking_duration: None,
                active_threshold: Some(ActiveThreshold::Percentage {
                    percent: Decimal::percent(50),
                }),
            },
            &[],
            "onft_voting",
            None,
        )
        .unwrap();

    // Get collection ID
    let onft_collection_id = query_config(&app, &module).unwrap().onft_collection_id;

    // Not active as none staked
    let is_active: IsActiveResponse = app
        .wrap()
        .query_wasm_smart(module.clone(), &QueryMsg::IsActive {})
        .unwrap();
    assert!(!is_active.active);

    // Stake 2 token as creator, should not be active.
    stake_nft(&mut app, &onft_collection_id, &module, STAKER, "1").unwrap();
    stake_nft(&mut app, &onft_collection_id, &module, STAKER, "2").unwrap();

    app.update_block(next_block);

    let is_active: IsActiveResponse = app
        .wrap()
        .query_wasm_smart(module.clone(), &QueryMsg::IsActive {})
        .unwrap();
    assert!(!is_active.active);

    // Stake 1 more token as creator, should now be active.
    stake_nft(&mut app, &onft_collection_id, &module, STAKER, "3").unwrap();
    app.update_block(next_block);

    let is_active: IsActiveResponse = app
        .wrap()
        .query_wasm_smart(module, &QueryMsg::IsActive {})
        .unwrap();
    assert!(is_active.active);
}

#[test]
fn test_update_active_threshold() {
    let CommonTest {
        mut app,
        module,
        nft,
        ..
    } = setup_test(None, None);

    mint_nft(&mut app, &nft, STAKER, "1").unwrap();

    let resp: ActiveThresholdResponse = app
        .wrap()
        .query_wasm_smart(module.clone(), &QueryMsg::ActiveThreshold {})
        .unwrap();
    assert_eq!(resp.active_threshold, None);

    let msg = ExecuteMsg::UpdateActiveThreshold {
        new_threshold: Some(ActiveThreshold::AbsoluteCount {
            count: Uint128::new(1),
        }),
    };

    // Expect failure as sender is not the DAO
    app.execute_contract(Addr::unchecked("bob"), module.clone(), &msg, &[])
        .unwrap_err();

    // Expect success as sender is the DAO
    app.execute_contract(Addr::unchecked(DAO), module.clone(), &msg, &[])
        .unwrap();

    let resp: ActiveThresholdResponse = app
        .wrap()
        .query_wasm_smart(module.clone(), &QueryMsg::ActiveThreshold {})
        .unwrap();
    assert_eq!(
        resp.active_threshold,
        Some(ActiveThreshold::AbsoluteCount {
            count: Uint128::new(1)
        })
    );

    app.execute_contract(
        Addr::unchecked(DAO),
        module.clone(),
        &ExecuteMsg::UpdateActiveThreshold {
            new_threshold: Some(ActiveThreshold::Percentage {
                percent: Decimal::percent(50),
            }),
        },
        &[],
    )
    .unwrap();

    let resp: ActiveThresholdResponse = app
        .wrap()
        .query_wasm_smart(module.clone(), &QueryMsg::ActiveThreshold {})
        .unwrap();
    assert_eq!(
        resp.active_threshold,
        Some(ActiveThreshold::Percentage {
            percent: Decimal::percent(50)
        })
    );

    // remove
    app.execute_contract(
        Addr::unchecked(DAO),
        module.clone(),
        &ExecuteMsg::UpdateActiveThreshold {
            new_threshold: None,
        },
        &[],
    )
    .unwrap();

    let resp: ActiveThresholdResponse = app
        .wrap()
        .query_wasm_smart(module.clone(), &QueryMsg::ActiveThreshold {})
        .unwrap();
    assert_eq!(resp.active_threshold, None);

    // verify is active
    let is_active: IsActiveResponse = app
        .wrap()
        .query_wasm_smart(module, &QueryMsg::IsActive {})
        .unwrap();
    assert!(is_active.active);
}

#[test]
#[should_panic(
    expected = "Active threshold percentage must be greater than 0 and not greater than 1"
)]
fn test_active_threshold_percentage_gt_100() {
    setup_test(
        None,
        Some(ActiveThreshold::Percentage {
            percent: Decimal::percent(120),
        }),
    );
}

#[test]
#[should_panic(
    expected = "Active threshold percentage must be greater than 0 and not greater than 1"
)]
fn test_active_threshold_percentage_lte_0() {
    setup_test(
        None,
        Some(ActiveThreshold::Percentage {
            percent: Decimal::percent(0),
        }),
    );
}

#[test]
pub fn test_migrate_update_version() {
    let mut deps = mock_dependencies();
    cw2::set_contract_version(&mut deps.storage, "my-contract", "1.0.0").unwrap();

    migrate(deps.as_mut(), mock_env(), MigrateMsg {}).unwrap();
    let version = cw2::get_contract_version(&deps.storage).unwrap();
    assert_eq!(version.version, CONTRACT_VERSION);
    assert_eq!(version.contract, CONTRACT_NAME);

    // migrate again, should do nothing
    migrate(deps.as_mut(), mock_env(), MigrateMsg {}).unwrap();
    let version = cw2::get_contract_version(&deps.storage).unwrap();
    assert_eq!(version.version, CONTRACT_VERSION);
    assert_eq!(version.contract, CONTRACT_NAME);
}

/// Staking tokens has a one block delay before staked tokens are
/// reflected in voting power. Unstaking tokens has a one block delay
/// before the unstaking is reflected in voting power, yet you have
/// access to the NFT. If I immediately stake an unstaked NFT, my
/// voting power should not change.
#[test]
fn test_circular_stake() -> anyhow::Result<()> {
    let CommonTest {
        mut app,
        module,
        nft,
        ..
    } = setup_test(None, None);

    mint_and_stake_nft(&mut app, &nft, &module, STAKER, "1")?;
    mint_and_stake_nft(&mut app, &nft, &module, STAKER, "2")?;

    app.update_block(next_block);

    let (total, voting) = query_total_and_voting_power(&app, &module, STAKER, None)?;
    assert_eq!(total, Uint128::new(2));
    assert_eq!(voting, Uint128::new(2));

    unstake_nfts(&mut app, &module, STAKER, &["1", "2"])?;

    // Unchanged, one block delay.
    let (total, voting) = query_total_and_voting_power(&app, &module, STAKER, None)?;
    assert_eq!(total, Uint128::new(2));
    assert_eq!(voting, Uint128::new(2));

    stake_nft(&mut app, &nft, &module, STAKER, "1")?;
    stake_nft(&mut app, &nft, &module, STAKER, "2")?;

    // Unchanged.
    let (total, voting) = query_total_and_voting_power(&app, &module, STAKER, None)?;
    assert_eq!(total, Uint128::new(2));
    assert_eq!(voting, Uint128::new(2));

    app.update_block(next_block);

    // Still unchanged.
    let (total, voting) = query_total_and_voting_power(&app, &module, STAKER, None)?;
    assert_eq!(total, Uint128::new(2));
    assert_eq!(voting, Uint128::new(2));

    Ok(())
}

/// I can immediately unstake after staking even though voting powers
/// aren't updated until one block later. Voting power does not change
/// if I do this.
#[test]
fn test_immediate_unstake() -> anyhow::Result<()> {
    let CommonTest {
        mut app,
        module,
        nft,
        ..
    } = setup_test(None, None);

    mint_and_stake_nft(&mut app, &nft, &module, STAKER, "1")?;
    mint_and_stake_nft(&mut app, &nft, &module, STAKER, "2")?;

    unstake_nfts(&mut app, &module, STAKER, &["1", "2"])?;

    app.update_block(next_block);

    let (total, voting) = query_total_and_voting_power(&app, &module, STAKER, None)?;
    assert_eq!(total, Uint128::zero());
    assert_eq!(voting, Uint128::zero());

    Ok(())
}

/// I cannot prepare/stake an NFT I do not own.
#[test]
fn test_stake_unowned() -> anyhow::Result<()> {
    let CommonTest {
        mut app,
        module,
        nft,
        ..
    } = setup_test(None, None);

    mint_nft(&mut app, &nft, STAKER, "1")?;

    let res = stake_nft(&mut app, &nft, &module, "other", "1");
    is_error!(res => "Only an NFT's owner can prepare it to be staked");

    app.update_block(next_block);
    let voting = query_voting_power(&app, &module, STAKER, None)?;
    assert_eq!(voting.power, Uint128::new(0));

    Ok(())
}

/// I cannot confirm a stake before preparing it.
#[test]
fn test_stake_unprepared() -> anyhow::Result<()> {
    let CommonTest {
        mut app,
        module,
        nft,
        ..
    } = setup_test(None, None);

    mint_nft(&mut app, &nft, STAKER, "1")?;

    // attempt confirm without preparing
    let res = confirm_stake_nft(&mut app, &module, STAKER, "1");
    is_error!(res => "NFTs must be prepared and transferred before they can be staked");

    app.update_block(next_block);
    let voting = query_voting_power(&app, &module, STAKER, None)?;
    assert_eq!(voting.power, Uint128::new(0));

    Ok(())
}

/// I cannot confirm a stake before preparing it and transferring NFT.
#[test]
fn test_stake_prepared_untransferred() -> anyhow::Result<()> {
    let CommonTest {
        mut app,
        module,
        nft,
        ..
    } = setup_test(None, None);

    mint_nft(&mut app, &nft, STAKER, "1")?;

    // prepare but don't transfer
    prepare_stake_nft(&mut app, &module, STAKER, "1")?;

    // attempt confirm
    let res = confirm_stake_nft(&mut app, &module, STAKER, "1");
    is_error!(res => "NFTs must be prepared and transferred before they can be staked");

    app.update_block(next_block);
    let voting = query_voting_power(&app, &module, STAKER, None)?;
    assert_eq!(voting.power, Uint128::new(0));

    Ok(())
}

/// I cannot confirm a stake that someone else prepared.
#[test]
fn test_stake_prepared_confirm_other_owner() -> anyhow::Result<()> {
    let CommonTest {
        mut app,
        module,
        nft,
        ..
    } = setup_test(None, None);

    mint_nft(&mut app, &nft, STAKER, "1")?;

    // prepare
    prepare_stake_nft(&mut app, &module, STAKER, "1")?;

    // transfer to voting contract
    send_nft(&mut app, &nft, "1", STAKER, module.as_str())?;

    // attempt confirm
    let res = confirm_stake_nft(&mut app, &module, "other", "1");
    is_error!(res => "NFTs must be prepared and transferred before they can be staked");

    app.update_block(next_block);
    let voting = query_voting_power(&app, &module, "other", None)?;
    assert_eq!(voting.power, Uint128::new(0));

    Ok(())
}

/// I can override a prepared stake.
#[test]
fn test_override_prepared() -> anyhow::Result<()> {
    let CommonTest {
        mut app,
        module,
        nft,
        ..
    } = setup_test(None, None);

    mint_nft(&mut app, &nft, STAKER, "1")?;

    // prepare
    prepare_stake_nft(&mut app, &module, STAKER, "1")?;

    // transfer to someone else
    send_nft(&mut app, &nft, "1", STAKER, "other")?;

    // override previous owner's prepare
    prepare_stake_nft(&mut app, &module, "other", "1")?;

    // transfer to voting contract
    send_nft(&mut app, &nft, "1", "other", module.as_str())?;

    // confirm
    confirm_stake_nft(&mut app, &module, "other", "1")?;

    app.update_block(next_block);
    let voting = query_voting_power(&app, &module, "other", None)?;
    assert_eq!(voting.power, Uint128::new(1));

    Ok(())
}

/// I can determine what my voting power _will_ be after staking by
/// asking for my voting power one block in the future.
#[test]
fn test_query_the_future() -> anyhow::Result<()> {
    let CommonTest {
        mut app,
        module,
        nft,
        ..
    } = setup_test(None, None);

    mint_and_stake_nft(&mut app, &nft, &module, STAKER, "1")?;

    // Future voting power will be one under current conditions.
    let voting = query_voting_power(&app, &module, STAKER, Some(app.block_info().height + 100))?;
    assert_eq!(voting.power, Uint128::new(1));

    // Current voting power is zero.
    let voting = query_voting_power(&app, &module, STAKER, None)?;
    assert_eq!(voting.power, Uint128::new(0));

    unstake_nfts(&mut app, &module, STAKER, &["1"])?;

    // Future voting power is now zero.
    let voting = query_voting_power(&app, &module, STAKER, Some(app.block_info().height + 100))?;
    assert_eq!(voting.power, Uint128::zero());

    Ok(())
}

/// I can cancel my own prepared stake.
#[test]
fn test_preparer_cancel_prepared_stake() -> anyhow::Result<()> {
    let CommonTest {
        mut app,
        module,
        nft,
        ..
    } = setup_test(None, None);

    mint_nft(&mut app, &nft, STAKER, "1")?;

    // prepare but don't transfer
    prepare_stake_nft(&mut app, &module, STAKER, "1")?;

    // cancel
    cancel_stake(&mut app, &module, STAKER, "1", None)?;

    // prepare and transfer
    prepare_stake_nft(&mut app, &module, STAKER, "1")?;
    send_nft(&mut app, &nft, "1", STAKER, module.as_str())?;

    // voting contract has the NFT
    let owner = query_nft_owner(&app, &nft, "1")?;
    assert_eq!(owner, module.to_string());

    // cancel
    cancel_stake(&mut app, &module, STAKER, "1", None)?;

    // original preparer has the NFT
    let owner = query_nft_owner(&app, &nft, "1")?;
    assert_eq!(owner, STAKER.to_string());

    // no voting power
    app.update_block(next_block);
    let voting = query_voting_power(&app, &module, STAKER, None)?;
    assert_eq!(voting.power, Uint128::new(0));

    Ok(())
}

/// I cannot cancel someone else's prepared stake, unless I own it.
#[test]
fn test_no_cancel_other_prepared_stake() -> anyhow::Result<()> {
    let CommonTest {
        mut app,
        module,
        nft,
        ..
    } = setup_test(None, None);

    mint_nft(&mut app, &nft, STAKER, "1")?;

    // prepare
    prepare_stake_nft(&mut app, &module, STAKER, "1")?;

    // cancel from other
    let res = cancel_stake(&mut app, &module, "other", "1", None);
    is_error!(res => "Only the owner or preparer can cancel a prepared stake");

    // transfer to other
    send_nft(&mut app, &nft, "1", STAKER, "other")?;
    // cancel from other
    cancel_stake(&mut app, &module, "other", "1", None)?;

    Ok(())
}

/// The DAO can cancel a prepared stake.
#[test]
fn test_dao_cancel_stake() -> anyhow::Result<()> {
    let CommonTest {
        mut app,
        module,
        nft,
        ..
    } = setup_test(None, None);

    mint_nft(&mut app, &nft, STAKER, "1")?;

    // prepare
    prepare_stake_nft(&mut app, &module, STAKER, "1")?;

    // cancel from DAO
    cancel_stake(&mut app, &module, DAO, "1", None)?;

    Ok(())
}

/// The DAO can cancel a prepared stake and send back to the preparer.
#[test]
fn test_dao_cancel_stake_and_return_to_preparer() -> anyhow::Result<()> {
    let CommonTest {
        mut app,
        module,
        nft,
        ..
    } = setup_test(None, None);

    mint_nft(&mut app, &nft, STAKER, "1")?;

    // prepare and transfer
    prepare_stake_nft(&mut app, &module, STAKER, "1")?;
    // transfer
    send_nft(&mut app, &nft, "1", STAKER, module.as_str())?;

    // voting contract has the NFT
    let owner = query_nft_owner(&app, &nft, "1")?;
    assert_eq!(owner, module.to_string());

    // cancel from DAO
    cancel_stake(&mut app, &module, DAO, "1", None)?;

    // preparer has the NFT
    let owner = query_nft_owner(&app, &nft, "1")?;
    assert_eq!(owner, STAKER);

    Ok(())
}

/// The DAO can cancel a prepared stake and send to anyone.
#[test]
fn test_dao_cancel_stake_and_send_to_anyone() -> anyhow::Result<()> {
    let CommonTest {
        mut app,
        module,
        nft,
        ..
    } = setup_test(None, None);

    mint_nft(&mut app, &nft, STAKER, "1")?;

    // prepare and transfer
    prepare_stake_nft(&mut app, &module, STAKER, "1")?;
    // transfer
    send_nft(&mut app, &nft, "1", STAKER, module.as_str())?;

    // voting contract has the NFT
    let owner = query_nft_owner(&app, &nft, "1")?;
    assert_eq!(owner, module.to_string());

    // cancel from DAO and send to other
    cancel_stake(&mut app, &module, DAO, "1", Some("other"))?;

    // other has the NFT
    let owner = query_nft_owner(&app, &nft, "1")?;
    assert_eq!(owner, "other");

    // other can stake
    stake_nft(&mut app, &nft, &module, "other", "1")?;

    Ok(())
}

/// The DAO must specify a recipient if no one prepared the NFT.
#[test]
fn test_dao_cancel_stake_must_have_recipient() -> anyhow::Result<()> {
    let CommonTest {
        mut app,
        module,
        nft,
        ..
    } = setup_test(None, None);

    mint_nft(&mut app, &nft, STAKER, "1")?;

    // transfer without preparing
    send_nft(&mut app, &nft, "1", STAKER, module.as_str())?;

    // voting contract has the NFT
    let owner = query_nft_owner(&app, &nft, "1")?;
    assert_eq!(owner, module.to_string());

    // cancel from DAO without prepared stake requires recipient
    let res = cancel_stake(&mut app, &module, DAO, "1", None);
    is_error!(res => "Recipient must be set when the DAO is cancelling a stake that was not prepared");

    // cancel from DAO and send back to staker
    cancel_stake(&mut app, &module, DAO, "1", Some(STAKER))?;

    // staker has the NFT
    let owner = query_nft_owner(&app, &nft, "1")?;
    assert_eq!(owner, STAKER);

    // staker can stake
    stake_nft(&mut app, &nft, &module, STAKER, "1")?;

    Ok(())
}
