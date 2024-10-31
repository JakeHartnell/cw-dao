use cosm_orc::orchestrator::SigningKey;
use cosmwasm_std::{Binary, Empty, Uint128};
use cw_utils::Duration;
use test_context::test_context;

use dao_voting_cw721_staked as module;

use crate::helpers::chain::Chain;

const CONTRACT_NAME: &str = "dao_voting_cw721_staked";
const CW721_NAME: &str = "cw721_base";

struct CommonTest {
    module: String,
    cw721: String,
}

pub fn instantiate_cw721_base(chain: &mut Chain, key: &SigningKey, minter: &str) -> String {
    chain
        .orc
        .instantiate(
            CW721_NAME,
            "instantiate_cw721_base",
            &cw721_base::InstantiateMsg {
                name: "bad kids".to_string(),
                symbol: "bad kids".to_string(),
                minter: minter.to_string(),
            },
            key,
            None,
            vec![],
        )
        .unwrap()
        .address
        .into()
}

fn setup_test(
    chain: &mut Chain,
    unstaking_duration: Option<Duration>,
    key: &SigningKey,
    minter: &str,
) -> CommonTest {
    let cw721 = instantiate_cw721_base(chain, key, minter);
    let module = chain
        .orc
        .instantiate(
            CONTRACT_NAME,
            "instantiate_dao_voting_cw721_staked",
            &module::msg::InstantiateMsg {
                nft_contract: module::msg::NftContract::Existing {
                    address: cw721.clone(),
                },
                unstaking_duration,
                active_threshold: None,
            },
            key,
            None,
            vec![],
        )
        .unwrap()
        .address
        .into();
    CommonTest { module, cw721 }
}

pub fn send_nft(
    chain: &mut Chain,
    sender: &SigningKey,
    receiver: &str,
    token_id: &str,
    msg: Binary,
) {
    chain
        .orc
        .execute(
            CW721_NAME,
            "stake_nft",
            &cw721::Cw721ExecuteMsg::SendNft {
                contract: receiver.to_string(),
                token_id: token_id.to_string(),
                msg,
            },
            sender,
            vec![],
        )
        .unwrap();
}

pub fn mint_nft(chain: &mut Chain, sender: &SigningKey, receiver: &str, token_id: &str) {
    chain
        .orc
        .execute(
            CW721_NAME,
            "mint_nft",
            &cw721_base::ExecuteMsg::Mint::<Empty, Empty> {
                token_id: token_id.to_string(),
                owner: receiver.to_string(),
                token_uri: None,
                extension: Empty::default(),
            },
            sender,
            vec![],
        )
        .unwrap();
}

pub fn unstake_nfts(chain: &mut Chain, sender: &SigningKey, token_ids: &[&str]) {
    chain
        .orc
        .execute(
            CONTRACT_NAME,
            "unstake_nfts",
            &module::msg::ExecuteMsg::Unstake {
                token_ids: token_ids.iter().map(|s| s.to_string()).collect(),
            },
            sender,
            vec![],
        )
        .unwrap();
}

pub fn claim_nfts(chain: &mut Chain, sender: &SigningKey) {
    chain
        .orc
        .execute(
            CONTRACT_NAME,
            "claim_nfts",
            &module::msg::ExecuteMsg::ClaimNfts {
                token_ids: Some(vec![]),
            },
            sender,
            vec![],
        )
        .unwrap();
}

pub fn query_voting_power(chain: &Chain, addr: &str, height: Option<u64>) -> Uint128 {
    let res = chain
        .orc
        .query(
            CONTRACT_NAME,
            &dao_interface::voting::Query::VotingPowerAtHeight {
                address: addr.to_string(),
                height,
            },
        )
        .unwrap();
    let data: dao_interface::voting::VotingPowerAtHeightResponse = res.data().unwrap();
    data.power
}

pub fn mint_and_stake_nft(
    chain: &mut Chain,
    sender_key: &SigningKey,
    sender: &str,
    module: &str,
    token_id: &str,
) {
    mint_nft(chain, sender_key, sender, token_id);
    send_nft(chain, sender_key, module, token_id, Binary::default());
}

#[test_context(Chain)]
#[test]
#[ignore]
fn cw721_stake_tokens(chain: &mut Chain) {
    let user_addr = chain.users["user1"].account.address.clone();
    let user_key = chain.users["user1"].key.clone();

    let CommonTest { module, .. } = setup_test(chain, None, &user_key, &user_addr);

    mint_and_stake_nft(chain, &user_key, &user_addr, &module, "a");

    // Wait for voting power to be updated.
    chain
        .orc
        .poll_for_n_blocks(1, core::time::Duration::from_millis(20_000), false)
        .unwrap();

    let voting_power = query_voting_power(chain, &user_addr, None);
    assert_eq!(voting_power, Uint128::new(1));

    unstake_nfts(chain, &user_key, &["a"]);

    chain
        .orc
        .poll_for_n_blocks(1, core::time::Duration::from_millis(20_000), false)
        .unwrap();

    let voting_power = query_voting_power(chain, &user_addr, None);
    assert_eq!(voting_power, Uint128::zero());
}
