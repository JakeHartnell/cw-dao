use cosmwasm_std::{Addr, Binary, Empty};
use cw721::Cw721ExecuteMsg;
use cw_multi_test::{App, AppResponse, Executor};

use anyhow::Result as AnyResult;
use cw_utils::Duration;

use crate::msg::{ClaimType, ExecuteMsg};

// Shorthand for an unchecked address.
macro_rules! addr {
    ($x:expr ) => {
        Addr::unchecked($x)
    };
}

pub fn send_nft(
    app: &mut App,
    cw721: &Addr,
    sender: &str,
    receiver: &Addr,
    token_id: &str,
    msg: Binary,
) -> AnyResult<AppResponse> {
    app.execute_contract(
        addr!(sender),
        cw721.clone(),
        &Cw721ExecuteMsg::SendNft {
            contract: receiver.to_string(),
            token_id: token_id.to_string(),
            msg,
        },
        &[],
    )
}

pub fn mint_nft(
    app: &mut App,
    cw721: &Addr,
    sender: &str,
    receiver: &str,
    token_id: &str,
) -> AnyResult<AppResponse> {
    app.execute_contract(
        addr!(sender),
        cw721.clone(),
        &cw721_base::ExecuteMsg::Mint::<Empty, Empty> {
            token_id: token_id.to_string(),
            owner: receiver.to_string(),
            token_uri: None,
            extension: Empty::default(),
        },
        &[],
    )
}

pub fn stake_nft(
    app: &mut App,
    cw721: &Addr,
    module: &Addr,
    sender: &str,
    token_id: &str,
) -> AnyResult<AppResponse> {
    send_nft(app, cw721, sender, module, token_id, Binary::default())
}

pub fn mint_and_stake_nft(
    app: &mut App,
    cw721: &Addr,
    module: &Addr,
    sender: &str,
    token_id: &str,
) -> AnyResult<()> {
    mint_nft(app, cw721, sender, sender, token_id)?;
    stake_nft(app, cw721, module, sender, token_id)?;
    Ok(())
}

pub fn unstake_nfts(
    app: &mut App,
    module: &Addr,
    sender: &str,
    token_ids: &[&str],
) -> AnyResult<AppResponse> {
    app.execute_contract(
        addr!(sender),
        module.clone(),
        &ExecuteMsg::Unstake {
            token_ids: token_ids.iter().map(|s| s.to_string()).collect(),
        },
        &[],
    )
}

pub fn update_config(
    app: &mut App,
    module: &Addr,
    sender: &str,
    duration: Option<Duration>,
) -> AnyResult<AppResponse> {
    app.execute_contract(
        addr!(sender),
        module.clone(),
        &ExecuteMsg::UpdateConfig { duration },
        &[],
    )
}

pub fn claim_nfts(app: &mut App, module: &Addr, sender: &str) -> AnyResult<AppResponse> {
    app.execute_contract(
        addr!(sender),
        module.clone(),
        &ExecuteMsg::ClaimNfts {
            r#type: ClaimType::All,
        },
        &[],
    )
}

pub fn claim_specific_nfts(
    app: &mut App,
    module: &Addr,
    sender: &str,
    token_ids: &[String],
) -> AnyResult<AppResponse> {
    app.execute_contract(
        addr!(sender),
        module.clone(),
        &ExecuteMsg::ClaimNfts {
            r#type: ClaimType::Specific(token_ids.to_vec()),
        },
        &[],
    )
}

pub fn claim_legacy_nfts(app: &mut App, module: &Addr, sender: &str) -> AnyResult<AppResponse> {
    app.execute_contract(
        addr!(sender),
        module.clone(),
        &ExecuteMsg::ClaimNfts {
            r#type: ClaimType::Legacy,
        },
        &[],
    )
}

pub fn add_hook(app: &mut App, module: &Addr, sender: &str, hook: &str) -> AnyResult<AppResponse> {
    app.execute_contract(
        addr!(sender),
        module.clone(),
        &ExecuteMsg::AddHook {
            addr: hook.to_string(),
        },
        &[],
    )
}

pub fn remove_hook(
    app: &mut App,
    module: &Addr,
    sender: &str,
    hook: &str,
) -> AnyResult<AppResponse> {
    app.execute_contract(
        addr!(sender),
        module.clone(),
        &ExecuteMsg::RemoveHook {
            addr: hook.to_string(),
        },
        &[],
    )
}
