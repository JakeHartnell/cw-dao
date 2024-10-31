use cosmwasm_std::storage_keys::to_length_prefixed_nested;
use cosmwasm_std::testing::{mock_dependencies, mock_env};
use cosmwasm_std::{
    to_json_binary, to_json_vec, Addr, Coin, Decimal, Empty, Storage, Uint128, WasmMsg,
};
use cw721_base::msg::{ExecuteMsg as Cw721ExecuteMsg, InstantiateMsg as Cw721InstantiateMsg};
use cw721_controllers::{NftClaim, NftClaimsResponse};
use cw_multi_test::{next_block, App, BankSudo, Executor, SudoMsg};
use cw_storage_plus::Map;
use cw_utils::Duration;
use dao_interface::voting::IsActiveResponse;
use dao_testing::contracts::{
    cw721_base_contract, dao_test_custom_factory_contract, dao_voting_cw721_staked_contract,
};
use dao_voting::threshold::{ActiveThreshold, ActiveThresholdResponse};

use crate::testing::execute::{claim_legacy_nfts, claim_specific_nfts};
use crate::{
    contract::{migrate, CONTRACT_NAME, CONTRACT_VERSION},
    msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, NftContract, QueryMsg},
    testing::{
        execute::{
            claim_nfts, mint_and_stake_nft, mint_nft, stake_nft, unstake_nfts, update_config,
        },
        queries::{query_config, query_hooks, query_nft_owner, query_total_and_voting_power},
    },
};

use super::instantiate::instantiate_cw721_base;
use super::{
    execute::{add_hook, remove_hook},
    is_error,
    queries::{query_claims, query_info, query_staked_nfts, query_total_power, query_voting_power},
    setup_test, CommonTest, CREATOR_ADDR,
};

// I can create new NFT collection when creating a dao-voting-cw721-staked contract
#[test]
fn test_instantiate_with_new_cw721_collection() -> anyhow::Result<()> {
    let mut app = App::default();
    let module_id = app.store_code(dao_voting_cw721_staked_contract());
    let cw721_id = app.store_code(cw721_base_contract());

    let module_addr = app
        .instantiate_contract(
            module_id,
            Addr::unchecked(CREATOR_ADDR),
            &InstantiateMsg {
                nft_contract: NftContract::New {
                    code_id: cw721_id,
                    label: "Test NFT".to_string(),
                    msg: to_json_binary(&Cw721InstantiateMsg {
                        name: "Test NFT".to_string(),
                        symbol: "TEST".to_string(),
                        minter: CREATOR_ADDR.to_string(),
                    })?,
                    initial_nfts: vec![to_json_binary(&Cw721ExecuteMsg::<Empty, Empty>::Mint {
                        owner: CREATOR_ADDR.to_string(),
                        token_uri: Some("https://example.com".to_string()),
                        token_id: "1".to_string(),
                        extension: Empty {},
                    })?],
                },
                unstaking_duration: None,
                active_threshold: None,
            },
            &[],
            "cw721_voting",
            None,
        )
        .unwrap();

    let config = query_config(&app, &module_addr)?;
    let cw721_addr = config.nft_address;

    // Check that the NFT contract was created
    let owner = query_nft_owner(&app, &cw721_addr, "1")?;
    assert_eq!(owner.owner, CREATOR_ADDR);

    Ok(())
}

// I can stake tokens, voting power and total power is updated one
// block later.
#[test]
fn test_stake_tokens() -> anyhow::Result<()> {
    let CommonTest {
        mut app,
        module,
        nft,
    } = setup_test(None);

    let total_power = query_total_power(&app, &module, None)?;
    let voting_power = query_voting_power(&app, &module, CREATOR_ADDR, None)?;

    assert_eq!(total_power.power, Uint128::zero());
    assert_eq!(total_power.height, app.block_info().height);

    assert_eq!(voting_power.power, Uint128::zero());
    assert_eq!(voting_power.height, app.block_info().height);

    mint_and_stake_nft(&mut app, &nft, &module, CREATOR_ADDR, "1")?;

    // Voting powers are not updated until a block has passed.
    let (total, personal) = query_total_and_voting_power(&app, &module, CREATOR_ADDR, None)?;
    assert!(total.is_zero());
    assert!(personal.is_zero());

    app.update_block(next_block);

    let (total, personal) = query_total_and_voting_power(&app, &module, CREATOR_ADDR, None)?;
    assert_eq!(total, Uint128::new(1));
    assert_eq!(personal, Uint128::new(1));

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
    } = setup_test(None);

    let friend = "friend";

    mint_and_stake_nft(&mut app, &nft, &module, CREATOR_ADDR, "1")?;
    mint_and_stake_nft(&mut app, &nft, &module, CREATOR_ADDR, "2")?;
    mint_and_stake_nft(&mut app, &nft, &module, CREATOR_ADDR, "3")?;

    mint_nft(&mut app, &nft, CREATOR_ADDR, friend, "4")?;
    mint_nft(&mut app, &nft, CREATOR_ADDR, friend, "5")?;
    stake_nft(&mut app, &nft, &module, friend, "4")?;
    stake_nft(&mut app, &nft, &module, friend, "5")?;

    app.update_block(next_block);

    let (total, personal) = query_total_and_voting_power(&app, &module, CREATOR_ADDR, None)?;
    assert_eq!(total, Uint128::new(5));
    assert_eq!(personal, Uint128::new(3));

    unstake_nfts(&mut app, &module, CREATOR_ADDR, &["1", "2"])?;

    // Voting power is updated when I unstake. Waits a block as it's a
    // snapshot map.
    let (total, personal) = query_total_and_voting_power(&app, &module, CREATOR_ADDR, None)?;
    assert_eq!(total, Uint128::new(5));
    assert_eq!(personal, Uint128::new(3));
    app.update_block(next_block);
    let (total, personal) = query_total_and_voting_power(&app, &module, CREATOR_ADDR, None)?;
    assert_eq!(total, Uint128::new(3));
    assert_eq!(personal, Uint128::new(1));

    // I can not unstake tokens I do not own. Anyhow can't figure out
    // how to downcast this error so we check for the expected string.
    let res = unstake_nfts(&mut app, &module, CREATOR_ADDR, &["4"]);
    is_error!(res => "Can not unstake that which you have not staked (unstaking 4)");

    let res = unstake_nfts(&mut app, &module, CREATOR_ADDR, &["5", "4"]);
    is_error!(res => "Can not unstake that which you have not staked (unstaking 5)");

    let res = unstake_nfts(&mut app, &module, CREATOR_ADDR, &["☯️", "4"]);
    is_error!(res => "Can not unstake that which you have not staked (unstaking ☯️)");

    // I can not unstake tokens more than once.
    let res = unstake_nfts(&mut app, &module, CREATOR_ADDR, &["1"]);
    is_error!(res => "Can not unstake that which you have not staked (unstaking 1)");

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
    } = setup_test(Some(Duration::Height(3)));

    mint_and_stake_nft(&mut app, &nft, &module, CREATOR_ADDR, "1")?;
    mint_and_stake_nft(&mut app, &nft, &module, CREATOR_ADDR, "2")?;

    unstake_nfts(&mut app, &module, CREATOR_ADDR, &["1"])?;

    let claims = query_claims(&app, &module, CREATOR_ADDR)?;
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
    let err = update_config(&mut app, &module, CREATOR_ADDR, Some(Duration::Time(0))).unwrap_err();
    assert_eq!(
        err.root_cause().to_string(),
        "Invalid unstaking duration, unstaking duration cannot be 0".to_string()
    );

    // Update duration
    update_config(&mut app, &module, CREATOR_ADDR, Some(Duration::Time(1)))?;

    // Existing claims should remain unchanged.
    let claims = query_claims(&app, &module, CREATOR_ADDR)?;
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
    unstake_nfts(&mut app, &module, CREATOR_ADDR, &["2"])?;
    let claims = query_claims(&app, &module, CREATOR_ADDR)?;
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
    claim_nfts(&mut app, &module, CREATOR_ADDR)?;
    let claims = query_claims(&app, &module, CREATOR_ADDR)?;
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
    } = setup_test(Some(Duration::Height(1)));

    mint_and_stake_nft(&mut app, &nft, &module, CREATOR_ADDR, "1")?;
    mint_and_stake_nft(&mut app, &nft, &module, CREATOR_ADDR, "2")?;
    mint_and_stake_nft(&mut app, &nft, &module, CREATOR_ADDR, "3")?;

    let claims = query_claims(&app, &module, CREATOR_ADDR)?;
    assert_eq!(claims.nft_claims, vec![]);

    let res = claim_nfts(&mut app, &module, CREATOR_ADDR);
    is_error!(res => "Nothing to claim");

    unstake_nfts(&mut app, &module, CREATOR_ADDR, &["2"])?;

    let claims = query_claims(&app, &module, CREATOR_ADDR)?;
    assert_eq!(
        claims.nft_claims,
        vec![NftClaim {
            token_id: "2".to_string(),
            release_at: cw_utils::Expiration::AtHeight(app.block_info().height + 1)
        }]
    );

    // Claim now exists, but is not yet expired. Nothing to claim.
    let res = claim_nfts(&mut app, &module, CREATOR_ADDR);
    is_error!(res => "Nothing to claim");

    app.update_block(next_block);
    claim_nfts(&mut app, &module, CREATOR_ADDR)?;

    let owner = query_nft_owner(&app, &nft, "2")?;
    assert_eq!(owner.owner, CREATOR_ADDR.to_string());

    Ok(())
}

// I can query and claim my pending legacy claims and non-legacy claims.
#[test]
pub fn test_legacy_claims_work() -> anyhow::Result<()> {
    let CommonTest {
        mut app,
        module,
        nft,
    } = setup_test(Some(Duration::Height(1)));

    mint_and_stake_nft(&mut app, &nft, &module, CREATOR_ADDR, "1")?;
    mint_and_stake_nft(&mut app, &nft, &module, CREATOR_ADDR, "2")?;
    mint_and_stake_nft(&mut app, &nft, &module, CREATOR_ADDR, "3")?;
    mint_and_stake_nft(&mut app, &nft, &module, CREATOR_ADDR, "4")?;
    mint_and_stake_nft(&mut app, &nft, &module, CREATOR_ADDR, "5")?;

    let claims = query_claims(&app, &module, CREATOR_ADDR)?;
    assert_eq!(claims.nft_claims, vec![]);

    let res = claim_legacy_nfts(&mut app, &module, CREATOR_ADDR);
    is_error!(res => "Nothing to claim");
    let res = claim_nfts(&mut app, &module, CREATOR_ADDR);
    is_error!(res => "Nothing to claim");

    // insert legacy claims manually

    // taken from cw-multi-test's WasmKeeper::contract_storage in wasm.rs
    let mut module_namespace = b"contract_data/".to_vec();
    module_namespace.extend_from_slice(module.as_bytes());
    let prefix = to_length_prefixed_nested(&[b"wasm", &module_namespace]);
    let key = Map::<&Addr, Vec<NftClaim>>::new("nft_claims").key(&Addr::unchecked(CREATOR_ADDR));
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

    let claims = query_claims(&app, &module, CREATOR_ADDR)?;
    assert_eq!(
        claims.nft_claims,
        vec![NftClaim {
            token_id: "4".to_string(),
            release_at: cw_utils::Expiration::AtHeight(app.block_info().height + 1)
        }]
    );

    // claim now exists, but is not yet expired. Nothing to claim.
    let res = claim_legacy_nfts(&mut app, &module, CREATOR_ADDR);
    is_error!(res => "Nothing to claim");
    let res = claim_nfts(&mut app, &module, CREATOR_ADDR);
    is_error!(res => "Nothing to claim");

    app.update_block(next_block);

    // no non-legacy claims
    let res = claim_nfts(&mut app, &module, CREATOR_ADDR);
    is_error!(res => "Nothing to claim");

    // legacy claim works
    claim_legacy_nfts(&mut app, &module, CREATOR_ADDR).unwrap();
    let owner = query_nft_owner(&app, &nft, "4")?;
    assert_eq!(owner.owner, CREATOR_ADDR.to_string());

    // unstake non-legacy
    unstake_nfts(&mut app, &module, CREATOR_ADDR, &["2"])?;

    let claims = query_claims(&app, &module, CREATOR_ADDR)?;
    assert_eq!(
        claims.nft_claims,
        vec![NftClaim {
            token_id: "2".to_string(),
            release_at: cw_utils::Expiration::AtHeight(app.block_info().height + 1)
        }]
    );

    // Claim now exists, but is not yet expired. Nothing to claim.
    let res = claim_legacy_nfts(&mut app, &module, CREATOR_ADDR);
    is_error!(res => "Nothing to claim");
    let res = claim_nfts(&mut app, &module, CREATOR_ADDR);
    is_error!(res => "Nothing to claim");

    app.update_block(next_block);

    // no legacy claims
    let res = claim_legacy_nfts(&mut app, &module, CREATOR_ADDR);
    is_error!(res => "Nothing to claim");

    claim_nfts(&mut app, &module, CREATOR_ADDR)?;

    let owner = query_nft_owner(&app, &nft, "2")?;
    assert_eq!(owner.owner, CREATOR_ADDR.to_string());

    // unstake another non-legacy
    unstake_nfts(&mut app, &module, CREATOR_ADDR, &["3"])?;

    let claims = query_claims(&app, &module, CREATOR_ADDR)?;
    assert_eq!(
        claims.nft_claims,
        vec![NftClaim {
            token_id: "3".to_string(),
            release_at: cw_utils::Expiration::AtHeight(app.block_info().height + 1)
        }]
    );

    app.update_block(next_block);

    claim_specific_nfts(&mut app, &module, CREATOR_ADDR, &["3".to_string()])?;
    let owner = query_nft_owner(&app, &nft, "3")?;
    assert_eq!(owner.owner, CREATOR_ADDR.to_string());

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
    unstake_nfts(&mut app, &module, CREATOR_ADDR, &["1"])?;

    let claims = query_claims(&app, &module, CREATOR_ADDR)?;
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
    claim_legacy_nfts(&mut app, &module, CREATOR_ADDR)?;
    claim_nfts(&mut app, &module, CREATOR_ADDR)?;

    let owner = query_nft_owner(&app, &nft, "1")?;
    assert_eq!(owner.owner, CREATOR_ADDR.to_string());
    let owner = query_nft_owner(&app, &nft, "5")?;
    assert_eq!(owner.owner, CREATOR_ADDR.to_string());

    // no claims left
    let claims = query_claims(&app, &module, CREATOR_ADDR)?;
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
    } = setup_test(Some(Duration::Height(1)));

    mint_and_stake_nft(&mut app, &nft, &module, CREATOR_ADDR, "1")?;
    mint_and_stake_nft(&mut app, &nft, &module, CREATOR_ADDR, "2")?;
    mint_and_stake_nft(&mut app, &nft, &module, CREATOR_ADDR, "3")?;

    let deardrie = "deardrie";
    mint_nft(&mut app, &nft, CREATOR_ADDR, deardrie, "4")?;
    mint_nft(&mut app, &nft, CREATOR_ADDR, deardrie, "5")?;

    let nfts = query_staked_nfts(&app, &module, deardrie, None, None)?;
    assert!(nfts.is_empty());

    stake_nft(&mut app, &nft, &module, deardrie, "4")?;
    stake_nft(&mut app, &nft, &module, deardrie, "5")?;

    let nfts = query_staked_nfts(&app, &module, deardrie, None, None)?;
    assert_eq!(nfts, vec!["4".to_string(), "5".to_string()]);

    let nfts = query_staked_nfts(&app, &module, CREATOR_ADDR, Some("1".to_string()), Some(0))?;
    assert!(nfts.is_empty());

    let nfts = query_staked_nfts(&app, &module, CREATOR_ADDR, Some("3".to_string()), None)?;
    assert!(nfts.is_empty());
    let nfts = query_staked_nfts(
        &app,
        &module,
        CREATOR_ADDR,
        Some("3".to_string()),
        Some(500),
    )?;
    assert!(nfts.is_empty());

    let nfts = query_staked_nfts(&app, &module, CREATOR_ADDR, Some("1".to_string()), Some(2))?;
    assert_eq!(nfts, vec!["2".to_string(), "3".to_string()]);

    unstake_nfts(&mut app, &module, CREATOR_ADDR, &["2"])?;
    let nfts = query_staked_nfts(&app, &module, CREATOR_ADDR, Some("1".to_string()), Some(2))?;
    assert_eq!(nfts, vec!["3".to_string()]);

    Ok(())
}

#[test]
fn test_info_query_works() -> anyhow::Result<()> {
    let CommonTest { app, module, .. } = setup_test(None);
    let info = query_info(&app, &module)?;
    assert_eq!(info.info.version, env!("CARGO_PKG_VERSION").to_string());
    Ok(())
}

// The owner may add and remove hooks.
#[test]
fn test_add_remove_hooks() -> anyhow::Result<()> {
    let CommonTest {
        mut app,
        module,
        nft,
    } = setup_test(None);

    add_hook(&mut app, &module, CREATOR_ADDR, "meow")?;
    remove_hook(&mut app, &module, CREATOR_ADDR, "meow")?;

    // Minting NFT works if no hooks
    mint_and_stake_nft(&mut app, &nft, &module, CREATOR_ADDR, "1").unwrap();

    // Add a hook to a fake contract called "meow"
    add_hook(&mut app, &module, CREATOR_ADDR, "meow")?;

    let hooks = query_hooks(&app, &module)?;
    assert_eq!(hooks.hooks, vec!["meow".to_string()]);

    // Minting / staking now doesn't work because meow isn't a contract
    // This failure means the hook is working
    mint_and_stake_nft(&mut app, &nft, &module, CREATOR_ADDR, "1").unwrap_err();

    let res = add_hook(&mut app, &module, CREATOR_ADDR, "meow");
    is_error!(res => "Given address already registered as a hook");

    let res = remove_hook(&mut app, &module, CREATOR_ADDR, "blue");
    is_error!(res => "Given address not registered as a hook");

    let res = add_hook(&mut app, &module, "ekez", "evil");
    is_error!(res => "Unauthorized");

    Ok(())
}

#[test]
fn test_instantiate_with_invalid_duration_fails() {
    let mut app = App::default();
    let module_id = app.store_code(dao_voting_cw721_staked_contract());
    let cw721_id = app.store_code(cw721_base_contract());

    let err = app
        .instantiate_contract(
            module_id,
            Addr::unchecked(CREATOR_ADDR),
            &InstantiateMsg {
                nft_contract: NftContract::New {
                    code_id: cw721_id,
                    label: "Test NFT".to_string(),
                    msg: to_json_binary(&Cw721InstantiateMsg {
                        name: "Test NFT".to_string(),
                        symbol: "TEST".to_string(),
                        minter: CREATOR_ADDR.to_string(),
                    })
                    .unwrap(),
                    initial_nfts: vec![to_json_binary(
                        &Cw721ExecuteMsg::<Empty, Empty>::Extension { msg: Empty {} },
                    )
                    .unwrap()],
                },
                unstaking_duration: None,
                active_threshold: None,
            },
            &[],
            "cw721_voting",
            None,
        )
        .unwrap_err();
    assert_eq!(
        err.root_cause().to_string(),
        "New NFT contract must be instantiated with at least one NFT".to_string()
    );
}

#[test]
#[should_panic(expected = "Active threshold count must be greater than zero")]
fn test_instantiate_zero_active_threshold_count() {
    let mut app = App::default();
    let cw721_id = app.store_code(cw721_base_contract());
    let module_id = app.store_code(dao_voting_cw721_staked_contract());

    app.instantiate_contract(
        module_id,
        Addr::unchecked(CREATOR_ADDR),
        &InstantiateMsg {
            nft_contract: NftContract::New {
                code_id: cw721_id,
                label: "Test NFT".to_string(),
                msg: to_json_binary(&Cw721InstantiateMsg {
                    name: "Test NFT".to_string(),
                    symbol: "TEST".to_string(),
                    minter: CREATOR_ADDR.to_string(),
                })
                .unwrap(),
                initial_nfts: vec![to_json_binary(&Cw721ExecuteMsg::<Empty, Empty>::Mint {
                    owner: CREATOR_ADDR.to_string(),
                    token_uri: Some("https://example.com".to_string()),
                    token_id: "1".to_string(),
                    extension: Empty {},
                })
                .unwrap()],
            },
            unstaking_duration: None,
            active_threshold: Some(ActiveThreshold::AbsoluteCount {
                count: Uint128::zero(),
            }),
        },
        &[],
        "cw721_voting",
        None,
    )
    .unwrap();
}

#[test]
#[should_panic(expected = "Absolute count threshold cannot be greater than the total token supply")]
fn test_instantiate_invalid_active_threshold_count_new_nft() {
    let mut app = App::default();
    let cw721_id = app.store_code(cw721_base_contract());
    let module_id = app.store_code(dao_voting_cw721_staked_contract());

    app.instantiate_contract(
        module_id,
        Addr::unchecked(CREATOR_ADDR),
        &InstantiateMsg {
            nft_contract: NftContract::New {
                code_id: cw721_id,
                label: "Test NFT".to_string(),
                msg: to_json_binary(&Cw721InstantiateMsg {
                    name: "Test NFT".to_string(),
                    symbol: "TEST".to_string(),
                    minter: CREATOR_ADDR.to_string(),
                })
                .unwrap(),
                initial_nfts: vec![to_json_binary(&Cw721ExecuteMsg::<Empty, Empty>::Mint {
                    owner: CREATOR_ADDR.to_string(),
                    token_uri: Some("https://example.com".to_string()),
                    token_id: "1".to_string(),
                    extension: Empty {},
                })
                .unwrap()],
            },
            unstaking_duration: None,
            active_threshold: Some(ActiveThreshold::AbsoluteCount {
                count: Uint128::new(100),
            }),
        },
        &[],
        "cw721_voting",
        None,
    )
    .unwrap();
}

#[test]
#[should_panic(expected = "Absolute count threshold cannot be greater than the total token supply")]
fn test_instantiate_invalid_active_threshold_count_existing_nft() {
    let mut app = App::default();
    let module_id = app.store_code(dao_voting_cw721_staked_contract());
    let cw721_addr = instantiate_cw721_base(&mut app, CREATOR_ADDR, CREATOR_ADDR);

    app.instantiate_contract(
        module_id,
        Addr::unchecked(CREATOR_ADDR),
        &InstantiateMsg {
            nft_contract: NftContract::Existing {
                address: cw721_addr.to_string(),
            },
            unstaking_duration: None,
            active_threshold: Some(ActiveThreshold::AbsoluteCount {
                count: Uint128::new(100),
            }),
        },
        &[],
        "cw721_voting",
        None,
    )
    .unwrap();
}

#[test]
fn test_active_threshold_absolute_count() {
    let mut app = App::default();
    let cw721_id = app.store_code(cw721_base_contract());
    let module_id = app.store_code(dao_voting_cw721_staked_contract());

    let voting_addr = app
        .instantiate_contract(
            module_id,
            Addr::unchecked(CREATOR_ADDR),
            &InstantiateMsg {
                nft_contract: NftContract::New {
                    code_id: cw721_id,
                    label: "Test NFT".to_string(),
                    msg: to_json_binary(&Cw721InstantiateMsg {
                        name: "Test NFT".to_string(),
                        symbol: "TEST".to_string(),
                        minter: CREATOR_ADDR.to_string(),
                    })
                    .unwrap(),
                    initial_nfts: vec![
                        to_json_binary(&Cw721ExecuteMsg::<Empty, Empty>::Mint {
                            owner: CREATOR_ADDR.to_string(),
                            token_uri: Some("https://example.com".to_string()),
                            token_id: "1".to_string(),
                            extension: Empty {},
                        })
                        .unwrap(),
                        to_json_binary(&Cw721ExecuteMsg::<Empty, Empty>::Mint {
                            owner: CREATOR_ADDR.to_string(),
                            token_uri: Some("https://example.com".to_string()),
                            token_id: "2".to_string(),
                            extension: Empty {},
                        })
                        .unwrap(),
                        to_json_binary(&Cw721ExecuteMsg::<Empty, Empty>::Mint {
                            owner: CREATOR_ADDR.to_string(),
                            token_uri: Some("https://example.com".to_string()),
                            token_id: "3".to_string(),
                            extension: Empty {},
                        })
                        .unwrap(),
                    ],
                },
                unstaking_duration: None,
                active_threshold: Some(ActiveThreshold::AbsoluteCount {
                    count: Uint128::new(3),
                }),
            },
            &[],
            "cw721_voting",
            None,
        )
        .unwrap();

    // Get NFT contract address
    let nft_addr = query_config(&app, &voting_addr).unwrap().nft_address;

    // Not active as none staked
    let is_active: IsActiveResponse = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::IsActive {})
        .unwrap();
    assert!(!is_active.active);

    // Stake NFTs
    stake_nft(&mut app, &nft_addr, &voting_addr, CREATOR_ADDR, "1").unwrap();
    stake_nft(&mut app, &nft_addr, &voting_addr, CREATOR_ADDR, "2").unwrap();
    stake_nft(&mut app, &nft_addr, &voting_addr, CREATOR_ADDR, "3").unwrap();

    app.update_block(next_block);

    // Active as enough staked
    let is_active: IsActiveResponse = app
        .wrap()
        .query_wasm_smart(voting_addr, &QueryMsg::IsActive {})
        .unwrap();
    assert!(is_active.active);
}

#[test]
fn test_active_threshold_percent() {
    let mut app = App::default();
    let cw721_id = app.store_code(cw721_base_contract());
    let module_id = app.store_code(dao_voting_cw721_staked_contract());

    let voting_addr = app
        .instantiate_contract(
            module_id,
            Addr::unchecked(CREATOR_ADDR),
            &InstantiateMsg {
                nft_contract: NftContract::New {
                    code_id: cw721_id,
                    label: "Test NFT".to_string(),
                    msg: to_json_binary(&Cw721InstantiateMsg {
                        name: "Test NFT".to_string(),
                        symbol: "TEST".to_string(),
                        minter: CREATOR_ADDR.to_string(),
                    })
                    .unwrap(),
                    initial_nfts: vec![to_json_binary(&Cw721ExecuteMsg::<Empty, Empty>::Mint {
                        owner: CREATOR_ADDR.to_string(),
                        token_uri: Some("https://example.com".to_string()),
                        token_id: "1".to_string(),
                        extension: Empty {},
                    })
                    .unwrap()],
                },
                unstaking_duration: None,
                active_threshold: Some(ActiveThreshold::Percentage {
                    percent: Decimal::percent(20),
                }),
            },
            &[],
            "cw721_voting",
            None,
        )
        .unwrap();

    // Get NFT contract address
    let nft_addr = query_config(&app, &voting_addr).unwrap().nft_address;

    // Not active as none staked
    let is_active: IsActiveResponse = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::IsActive {})
        .unwrap();
    assert!(!is_active.active);

    // Stake NFTs
    stake_nft(&mut app, &nft_addr, &voting_addr, CREATOR_ADDR, "1").unwrap();
    app.update_block(next_block);

    // Active as enough staked
    let is_active: IsActiveResponse = app
        .wrap()
        .query_wasm_smart(voting_addr, &QueryMsg::IsActive {})
        .unwrap();
    assert!(is_active.active);
}

#[test]
fn test_active_threshold_percent_rounds_up() {
    let mut app = App::default();
    let cw721_id = app.store_code(cw721_base_contract());
    let module_id = app.store_code(dao_voting_cw721_staked_contract());

    let voting_addr = app
        .instantiate_contract(
            module_id,
            Addr::unchecked(CREATOR_ADDR),
            &InstantiateMsg {
                nft_contract: NftContract::New {
                    code_id: cw721_id,
                    label: "Test NFT".to_string(),
                    msg: to_json_binary(&Cw721InstantiateMsg {
                        name: "Test NFT".to_string(),
                        symbol: "TEST".to_string(),
                        minter: CREATOR_ADDR.to_string(),
                    })
                    .unwrap(),
                    initial_nfts: vec![
                        to_json_binary(&Cw721ExecuteMsg::<Empty, Empty>::Mint {
                            owner: CREATOR_ADDR.to_string(),
                            token_uri: Some("https://example.com".to_string()),
                            token_id: "1".to_string(),
                            extension: Empty {},
                        })
                        .unwrap(),
                        to_json_binary(&Cw721ExecuteMsg::<Empty, Empty>::Mint {
                            owner: CREATOR_ADDR.to_string(),
                            token_uri: Some("https://example.com".to_string()),
                            token_id: "2".to_string(),
                            extension: Empty {},
                        })
                        .unwrap(),
                        to_json_binary(&Cw721ExecuteMsg::<Empty, Empty>::Mint {
                            owner: CREATOR_ADDR.to_string(),
                            token_uri: Some("https://example.com".to_string()),
                            token_id: "3".to_string(),
                            extension: Empty {},
                        })
                        .unwrap(),
                        to_json_binary(&Cw721ExecuteMsg::<Empty, Empty>::Mint {
                            owner: CREATOR_ADDR.to_string(),
                            token_uri: Some("https://example.com".to_string()),
                            token_id: "4".to_string(),
                            extension: Empty {},
                        })
                        .unwrap(),
                        to_json_binary(&Cw721ExecuteMsg::<Empty, Empty>::Mint {
                            owner: CREATOR_ADDR.to_string(),
                            token_uri: Some("https://example.com".to_string()),
                            token_id: "5".to_string(),
                            extension: Empty {},
                        })
                        .unwrap(),
                    ],
                },
                unstaking_duration: None,
                active_threshold: Some(ActiveThreshold::Percentage {
                    percent: Decimal::percent(50),
                }),
            },
            &[],
            "cw721_voting",
            None,
        )
        .unwrap();

    // Get NFT contract address
    let nft_addr = query_config(&app, &voting_addr).unwrap().nft_address;

    // Not active as none staked
    let is_active: IsActiveResponse = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::IsActive {})
        .unwrap();
    assert!(!is_active.active);

    // Stake 2 token as creator, should not be active.
    stake_nft(&mut app, &nft_addr, &voting_addr, CREATOR_ADDR, "1").unwrap();
    stake_nft(&mut app, &nft_addr, &voting_addr, CREATOR_ADDR, "2").unwrap();

    app.update_block(next_block);

    let is_active: IsActiveResponse = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::IsActive {})
        .unwrap();
    assert!(!is_active.active);

    // Stake 1 more token as creator, should now be active.
    stake_nft(&mut app, &nft_addr, &voting_addr, CREATOR_ADDR, "3").unwrap();
    app.update_block(next_block);

    let is_active: IsActiveResponse = app
        .wrap()
        .query_wasm_smart(voting_addr, &QueryMsg::IsActive {})
        .unwrap();
    assert!(is_active.active);
}

#[test]
fn test_update_active_threshold() {
    let mut app = App::default();
    let cw721_id = app.store_code(cw721_base_contract());
    let module_id = app.store_code(dao_voting_cw721_staked_contract());

    let voting_addr = app
        .instantiate_contract(
            module_id,
            Addr::unchecked(CREATOR_ADDR),
            &InstantiateMsg {
                nft_contract: NftContract::New {
                    code_id: cw721_id,
                    label: "Test NFT".to_string(),
                    msg: to_json_binary(&Cw721InstantiateMsg {
                        name: "Test NFT".to_string(),
                        symbol: "TEST".to_string(),
                        minter: CREATOR_ADDR.to_string(),
                    })
                    .unwrap(),
                    initial_nfts: vec![to_json_binary(&Cw721ExecuteMsg::<Empty, Empty>::Mint {
                        owner: CREATOR_ADDR.to_string(),
                        token_uri: Some("https://example.com".to_string()),
                        token_id: "1".to_string(),
                        extension: Empty {},
                    })
                    .unwrap()],
                },
                unstaking_duration: None,
                active_threshold: None,
            },
            &[],
            "cw721_voting",
            None,
        )
        .unwrap();

    let resp: ActiveThresholdResponse = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::ActiveThreshold {})
        .unwrap();
    assert_eq!(resp.active_threshold, None);

    let msg = ExecuteMsg::UpdateActiveThreshold {
        new_threshold: Some(ActiveThreshold::AbsoluteCount {
            count: Uint128::new(1),
        }),
    };

    // Expect failure as sender is not the DAO
    app.execute_contract(Addr::unchecked("bob"), voting_addr.clone(), &msg, &[])
        .unwrap_err();

    // Expect success as sender is the DAO (in this case the creator)
    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        voting_addr.clone(),
        &msg,
        &[],
    )
    .unwrap();

    let resp: ActiveThresholdResponse = app
        .wrap()
        .query_wasm_smart(voting_addr, &QueryMsg::ActiveThreshold {})
        .unwrap();
    assert_eq!(
        resp.active_threshold,
        Some(ActiveThreshold::AbsoluteCount {
            count: Uint128::new(1)
        })
    );
}

#[test]
#[should_panic(
    expected = "Active threshold percentage must be greater than 0 and not greater than 1"
)]
fn test_active_threshold_percentage_gt_100() {
    let mut app = App::default();
    let cw721_id = app.store_code(cw721_base_contract());
    let module_id = app.store_code(dao_voting_cw721_staked_contract());

    app.instantiate_contract(
        module_id,
        Addr::unchecked(CREATOR_ADDR),
        &InstantiateMsg {
            nft_contract: NftContract::New {
                code_id: cw721_id,
                label: "Test NFT".to_string(),
                msg: to_json_binary(&Cw721InstantiateMsg {
                    name: "Test NFT".to_string(),
                    symbol: "TEST".to_string(),
                    minter: CREATOR_ADDR.to_string(),
                })
                .unwrap(),
                initial_nfts: vec![to_json_binary(&Cw721ExecuteMsg::<Empty, Empty>::Mint {
                    owner: CREATOR_ADDR.to_string(),
                    token_uri: Some("https://example.com".to_string()),
                    token_id: "1".to_string(),
                    extension: Empty {},
                })
                .unwrap()],
            },
            unstaking_duration: None,
            active_threshold: Some(ActiveThreshold::Percentage {
                percent: Decimal::percent(120),
            }),
        },
        &[],
        "cw721_voting",
        None,
    )
    .unwrap();
}

#[test]
#[should_panic(
    expected = "Active threshold percentage must be greater than 0 and not greater than 1"
)]
fn test_active_threshold_percentage_lte_0() {
    let mut app = App::default();
    let cw721_id = app.store_code(cw721_base_contract());
    let module_id = app.store_code(dao_voting_cw721_staked_contract());

    app.instantiate_contract(
        module_id,
        Addr::unchecked(CREATOR_ADDR),
        &InstantiateMsg {
            nft_contract: NftContract::New {
                code_id: cw721_id,
                label: "Test NFT".to_string(),
                msg: to_json_binary(&Cw721InstantiateMsg {
                    name: "Test NFT".to_string(),
                    symbol: "TEST".to_string(),
                    minter: CREATOR_ADDR.to_string(),
                })
                .unwrap(),
                initial_nfts: vec![to_json_binary(&Cw721ExecuteMsg::<Empty, Empty>::Mint {
                    owner: CREATOR_ADDR.to_string(),
                    token_uri: Some("https://example.com".to_string()),
                    token_id: "1".to_string(),
                    extension: Empty {},
                })
                .unwrap()],
            },
            unstaking_duration: None,
            active_threshold: Some(ActiveThreshold::Percentage {
                percent: Decimal::percent(0),
            }),
        },
        &[],
        "cw721_voting",
        None,
    )
    .unwrap();
}

#[test]
fn test_invalid_instantiate_msg() {
    let mut app = App::default();
    let module_id = app.store_code(dao_voting_cw721_staked_contract());
    let cw721_id = app.store_code(cw721_base_contract());

    let err = app
        .instantiate_contract(
            module_id,
            Addr::unchecked(CREATOR_ADDR),
            &InstantiateMsg {
                nft_contract: NftContract::New {
                    code_id: cw721_id,
                    label: "Test NFT".to_string(),
                    msg: to_json_binary(&Empty {}).unwrap(),
                    initial_nfts: vec![to_json_binary(&Cw721ExecuteMsg::<Empty, Empty>::Mint {
                        owner: CREATOR_ADDR.to_string(),
                        token_uri: Some("https://example.com".to_string()),
                        token_id: "1".to_string(),
                        extension: Empty {},
                    })
                    .unwrap()],
                },
                unstaking_duration: None,
                active_threshold: Some(ActiveThreshold::AbsoluteCount {
                    count: Uint128::new(1),
                }),
            },
            &[],
            "cw721_voting",
            None,
        )
        .unwrap_err();
    assert_eq!(
        err.root_cause().to_string(),
        "Error instantiating NFT contract".to_string()
    );
}

#[test]
fn test_invalid_initial_nft_msg() {
    let mut app = App::default();
    let module_id = app.store_code(dao_voting_cw721_staked_contract());
    let cw721_id = app.store_code(cw721_base_contract());

    let err = app
        .instantiate_contract(
            module_id,
            Addr::unchecked(CREATOR_ADDR),
            &InstantiateMsg {
                nft_contract: NftContract::New {
                    code_id: cw721_id,
                    label: "Test NFT".to_string(),
                    msg: to_json_binary(&Cw721InstantiateMsg {
                        name: "Test NFT".to_string(),
                        symbol: "TEST".to_string(),
                        minter: CREATOR_ADDR.to_string(),
                    })
                    .unwrap(),
                    initial_nfts: vec![to_json_binary(
                        &Cw721ExecuteMsg::<Empty, Empty>::Extension { msg: Empty {} },
                    )
                    .unwrap()],
                },
                unstaking_duration: None,
                active_threshold: None,
            },
            &[],
            "cw721_voting",
            None,
        )
        .unwrap_err();
    assert_eq!(
        err.root_cause().to_string(),
        "New NFT contract must be instantiated with at least one NFT".to_string()
    );
}

#[test]
fn test_invalid_initial_nft_msg_wrong_absolute_count() {
    let mut app = App::default();
    let module_id = app.store_code(dao_voting_cw721_staked_contract());
    let cw721_id = app.store_code(cw721_base_contract());

    let err = app
        .instantiate_contract(
            module_id,
            Addr::unchecked(CREATOR_ADDR),
            &InstantiateMsg {
                nft_contract: NftContract::New {
                    code_id: cw721_id,
                    label: "Test NFT".to_string(),
                    msg: to_json_binary(&Cw721InstantiateMsg {
                        name: "Test NFT".to_string(),
                        symbol: "TEST".to_string(),
                        minter: CREATOR_ADDR.to_string(),
                    })
                    .unwrap(),
                    initial_nfts: vec![
                        to_json_binary(&Cw721ExecuteMsg::<Empty, Empty>::Extension {
                            msg: Empty {},
                        })
                        .unwrap(),
                        to_json_binary(&Cw721ExecuteMsg::<Empty, Empty>::Mint {
                            owner: CREATOR_ADDR.to_string(),
                            token_uri: Some("https://example.com".to_string()),
                            token_id: "1".to_string(),
                            extension: Empty {},
                        })
                        .unwrap(),
                    ],
                },
                unstaking_duration: None,
                active_threshold: Some(ActiveThreshold::AbsoluteCount {
                    count: Uint128::new(2),
                }),
            },
            &[],
            "cw721_voting",
            None,
        )
        .unwrap_err();
    assert_eq!(
        err.root_cause().to_string(),
        "Absolute count threshold cannot be greater than the total token supply".to_string()
    );
}

#[test]
fn test_no_initial_nfts_fails() {
    let mut app = App::default();
    let cw721_id = app.store_code(cw721_base_contract());
    let module_id = app.store_code(dao_voting_cw721_staked_contract());

    let err = app
        .instantiate_contract(
            module_id,
            Addr::unchecked(CREATOR_ADDR),
            &InstantiateMsg {
                nft_contract: NftContract::New {
                    code_id: cw721_id,
                    label: "Test NFT".to_string(),
                    msg: to_json_binary(&Cw721InstantiateMsg {
                        name: "Test NFT".to_string(),
                        symbol: "TEST".to_string(),
                        minter: CREATOR_ADDR.to_string(),
                    })
                    .unwrap(),
                    initial_nfts: vec![],
                },
                unstaking_duration: None,
                active_threshold: Some(ActiveThreshold::Percentage {
                    percent: Decimal::percent(1),
                }),
            },
            &[],
            "cw721_voting",
            None,
        )
        .unwrap_err();
    assert_eq!(
        err.root_cause().to_string(),
        "New NFT contract must be instantiated with at least one NFT".to_string()
    );
}

#[test]
fn test_factory() {
    let mut app = App::default();
    let module_id = app.store_code(dao_voting_cw721_staked_contract());
    let cw721_id = app.store_code(cw721_base_contract());
    let factory_id = app.store_code(dao_test_custom_factory_contract());

    // Instantiate factory
    let factory_addr = app
        .instantiate_contract(
            factory_id,
            Addr::unchecked(CREATOR_ADDR),
            &dao_test_custom_factory::msg::InstantiateMsg {},
            &[],
            "test factory".to_string(),
            None,
        )
        .unwrap();

    // Instantiate using factory succeeds
    app.instantiate_contract(
        module_id,
        Addr::unchecked(CREATOR_ADDR),
        &InstantiateMsg {
            nft_contract: NftContract::Factory(
                to_json_binary(&WasmMsg::Execute {
                    contract_addr: factory_addr.to_string(),
                    msg: to_json_binary(&dao_test_custom_factory::msg::ExecuteMsg::NftFactory {
                        code_id: cw721_id,
                        cw721_instantiate_msg: Cw721InstantiateMsg {
                            name: "Test NFT".to_string(),
                            symbol: "TEST".to_string(),
                            minter: CREATOR_ADDR.to_string(),
                        },
                        initial_nfts: vec![],
                    })
                    .unwrap(),
                    funds: vec![],
                })
                .unwrap(),
            ),
            unstaking_duration: None,
            active_threshold: Some(ActiveThreshold::Percentage {
                percent: Decimal::percent(1),
            }),
        },
        &[],
        "cw721_voting",
        None,
    )
    .unwrap();
}

#[test]
fn test_factory_with_funds_pass_through() {
    let mut app = App::default();
    let module_id = app.store_code(dao_voting_cw721_staked_contract());
    let cw721_id = app.store_code(cw721_base_contract());
    let factory_id = app.store_code(dao_test_custom_factory_contract());

    // Mint some tokens to creator
    app.sudo(SudoMsg::Bank(BankSudo::Mint {
        to_address: CREATOR_ADDR.to_string(),
        amount: vec![Coin {
            denom: "ujuno".to_string(),
            amount: Uint128::new(10000),
        }],
    }))
    .unwrap();

    // Instantiate factory
    let factory_addr = app
        .instantiate_contract(
            factory_id,
            Addr::unchecked(CREATOR_ADDR),
            &dao_test_custom_factory::msg::InstantiateMsg {},
            &[],
            "test factory".to_string(),
            None,
        )
        .unwrap();

    // Instantiate without funds fails
    app.instantiate_contract(
        module_id,
        Addr::unchecked(CREATOR_ADDR),
        &InstantiateMsg {
            nft_contract: NftContract::Factory(
                to_json_binary(&WasmMsg::Execute {
                    contract_addr: factory_addr.to_string(),
                    msg: to_json_binary(
                        &dao_test_custom_factory::msg::ExecuteMsg::NftFactoryWithFunds {
                            code_id: cw721_id,
                            cw721_instantiate_msg: Cw721InstantiateMsg {
                                name: "Test NFT".to_string(),
                                symbol: "TEST".to_string(),
                                minter: CREATOR_ADDR.to_string(),
                            },
                            initial_nfts: vec![to_json_binary(
                                &Cw721ExecuteMsg::<Empty, Empty>::Mint {
                                    owner: CREATOR_ADDR.to_string(),
                                    token_uri: Some("https://example.com".to_string()),
                                    token_id: "1".to_string(),
                                    extension: Empty {},
                                },
                            )
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
        },
        &[],
        "cw721_voting",
        None,
    )
    .unwrap_err();

    // Instantiate using factory succeeds
    let funds = vec![Coin {
        denom: "ujuno".to_string(),
        amount: Uint128::new(100),
    }];
    app.instantiate_contract(
        module_id,
        Addr::unchecked(CREATOR_ADDR),
        &InstantiateMsg {
            nft_contract: NftContract::Factory(
                to_json_binary(&WasmMsg::Execute {
                    contract_addr: factory_addr.to_string(),
                    msg: to_json_binary(
                        &dao_test_custom_factory::msg::ExecuteMsg::NftFactoryWithFunds {
                            code_id: cw721_id,
                            cw721_instantiate_msg: Cw721InstantiateMsg {
                                name: "Test NFT".to_string(),
                                symbol: "TEST".to_string(),
                                minter: CREATOR_ADDR.to_string(),
                            },
                            initial_nfts: vec![to_json_binary(
                                &Cw721ExecuteMsg::<Empty, Empty>::Mint {
                                    owner: CREATOR_ADDR.to_string(),
                                    token_uri: Some("https://example.com".to_string()),
                                    token_id: "1".to_string(),
                                    extension: Empty {},
                                },
                            )
                            .unwrap()],
                        },
                    )
                    .unwrap(),
                    funds: funds.clone(),
                })
                .unwrap(),
            ),
            unstaking_duration: None,
            active_threshold: Some(ActiveThreshold::Percentage {
                percent: Decimal::percent(1),
            }),
        },
        &funds,
        "cw721_voting",
        None,
    )
    .unwrap();
}

#[test]
#[should_panic(expected = "Factory message must serialize to WasmMsg::Execute")]
fn test_unsupported_factory_msg() {
    let mut app = App::default();
    let module_id = app.store_code(dao_voting_cw721_staked_contract());
    let cw721_id = app.store_code(cw721_base_contract());

    // Instantiate using factory succeeds
    app.instantiate_contract(
        module_id,
        Addr::unchecked(CREATOR_ADDR),
        &InstantiateMsg {
            nft_contract: NftContract::Factory(
                to_json_binary(&WasmMsg::Instantiate {
                    code_id: cw721_id,
                    msg: to_json_binary(&dao_test_custom_factory::msg::ExecuteMsg::NftFactory {
                        code_id: cw721_id,
                        cw721_instantiate_msg: Cw721InstantiateMsg {
                            name: "Test NFT".to_string(),
                            symbol: "TEST".to_string(),
                            minter: CREATOR_ADDR.to_string(),
                        },
                        initial_nfts: vec![],
                    })
                    .unwrap(),
                    admin: None,
                    label: "Test NFT".to_string(),
                    funds: vec![],
                })
                .unwrap(),
            ),
            unstaking_duration: None,
            active_threshold: Some(ActiveThreshold::Percentage {
                percent: Decimal::percent(1),
            }),
        },
        &[],
        "cw721_voting",
        None,
    )
    .unwrap();
}

#[test]
#[should_panic(
    expected = "Error parsing into type dao_interface::nft::NftFactoryCallback: unknown field `denom`, expected `nft_contract`"
)]
fn test_factory_wrong_callback() {
    let mut app = App::default();
    let module_id = app.store_code(dao_voting_cw721_staked_contract());
    let _cw721_id = app.store_code(cw721_base_contract());
    let factory_id = app.store_code(dao_test_custom_factory_contract());

    // Instantiate factory
    let factory_addr = app
        .instantiate_contract(
            factory_id,
            Addr::unchecked(CREATOR_ADDR),
            &dao_test_custom_factory::msg::InstantiateMsg {},
            &[],
            "test factory".to_string(),
            None,
        )
        .unwrap();

    // Instantiate using factory succeeds
    app.instantiate_contract(
        module_id,
        Addr::unchecked(CREATOR_ADDR),
        &InstantiateMsg {
            nft_contract: NftContract::Factory(
                to_json_binary(&WasmMsg::Execute {
                    contract_addr: factory_addr.to_string(),
                    msg: to_json_binary(
                        &dao_test_custom_factory::msg::ExecuteMsg::NftFactoryWrongCallback {},
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
        },
        &[],
        "cw721_voting",
        None,
    )
    .unwrap();
}

#[test]
#[should_panic(expected = "Invalid reply from sub-message: Missing reply data")]
fn test_factory_no_callback() {
    let mut app = App::default();
    let module_id = app.store_code(dao_voting_cw721_staked_contract());
    let _cw721_id = app.store_code(cw721_base_contract());
    let factory_id = app.store_code(dao_test_custom_factory_contract());

    // Instantiate factory
    let factory_addr = app
        .instantiate_contract(
            factory_id,
            Addr::unchecked(CREATOR_ADDR),
            &dao_test_custom_factory::msg::InstantiateMsg {},
            &[],
            "test factory".to_string(),
            None,
        )
        .unwrap();

    // Instantiate using factory succeeds
    app.instantiate_contract(
        module_id,
        Addr::unchecked(CREATOR_ADDR),
        &InstantiateMsg {
            nft_contract: NftContract::Factory(
                to_json_binary(&WasmMsg::Execute {
                    contract_addr: factory_addr.to_string(),
                    msg: to_json_binary(
                        &dao_test_custom_factory::msg::ExecuteMsg::NftFactoryNoCallback {},
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
        },
        &[],
        "cw721_voting",
        None,
    )
    .unwrap();
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
