use crate::msg::ExecuteMsg;
use anyhow::Result as AnyResult;
use cosmwasm_std::Addr;
use cw_multi_test::AppResponse;
use cw_multi_test::Executor;
use cw_utils::Duration;
use omniflix_std::types::omniflix::onft::v1beta1::{MsgCreateDenom, MsgMintOnft, MsgTransferOnft};

use super::app::OmniflixApp;
use super::DAO;

// Shorthand for an unchecked address.
macro_rules! addr {
    ($x:expr ) => {
        Addr::unchecked($x)
    };
}

pub fn create_onft_collection(
    app: &mut OmniflixApp,
    id: &str,
    sender: &str,
    minter: &str,
) -> String {
    app.execute(
        addr!(sender),
        MsgCreateDenom {
            id: id.to_string(),
            symbol: "BAD".to_string(),
            name: "Bad Kids".to_string(),
            description: "bad kids".to_string(),
            preview_uri: "".to_string(),
            schema: "".to_string(),
            sender: minter.to_string(),
            creation_fee: None,
            uri: "".to_string(),
            uri_hash: "".to_string(),
            data: "".to_string(),
            royalty_receivers: vec![],
        }
        .into(),
    )
    .unwrap();

    id.to_string()
}

pub fn mint_nft(
    app: &mut OmniflixApp,
    collection_id: &str,
    receiver: &str,
    token_id: &str,
) -> AnyResult<AppResponse> {
    app.execute(
        addr!(DAO),
        MsgMintOnft {
            id: token_id.to_string(),
            denom_id: collection_id.to_string(),
            metadata: None,
            data: "".to_string(),
            transferable: true,
            extensible: false,
            nsfw: false,
            royalty_share: "".to_string(),
            sender: DAO.to_string(),
            recipient: receiver.to_string(),
        }
        .into(),
    )
}

pub fn send_nft(
    app: &mut OmniflixApp,
    collection_id: &str,
    token_id: &str,
    sender: &str,
    recipient: &str,
) -> AnyResult<AppResponse> {
    app.execute(
        addr!(sender),
        MsgTransferOnft {
            denom_id: collection_id.to_string(),
            id: token_id.to_string(),
            sender: sender.to_string(),
            recipient: recipient.to_string(),
        }
        .into(),
    )
}

pub fn prepare_stake_nft(
    app: &mut OmniflixApp,
    module: &Addr,
    sender: &str,
    token_id: &str,
) -> AnyResult<AppResponse> {
    app.execute_contract(
        addr!(sender),
        module.clone(),
        &ExecuteMsg::PrepareStake {
            token_ids: vec![token_id.to_string()],
        },
        &[],
    )
}

pub fn confirm_stake_nft(
    app: &mut OmniflixApp,
    module: &Addr,
    sender: &str,
    token_id: &str,
) -> AnyResult<AppResponse> {
    app.execute_contract(
        addr!(sender),
        module.clone(),
        &ExecuteMsg::ConfirmStake {
            token_ids: vec![token_id.to_string()],
        },
        &[],
    )
}

pub fn stake_nft(
    app: &mut OmniflixApp,
    collection_id: &str,
    module: &Addr,
    sender: &str,
    token_id: &str,
) -> AnyResult<()> {
    prepare_stake_nft(app, module, sender, token_id)?;
    send_nft(app, collection_id, token_id, sender, module.as_str())?;
    confirm_stake_nft(app, module, sender, token_id)?;
    Ok(())
}

pub fn cancel_stake(
    app: &mut OmniflixApp,
    module: &Addr,
    sender: &str,
    token_id: &str,
    recipient: Option<&str>,
) -> AnyResult<AppResponse> {
    app.execute_contract(
        addr!(sender),
        module.clone(),
        &ExecuteMsg::CancelStake {
            token_ids: vec![token_id.to_string()],
            recipient: recipient.map(|s| s.to_string()),
        },
        &[],
    )
}

pub fn mint_and_stake_nft(
    app: &mut OmniflixApp,
    collection_id: &str,
    module: &Addr,
    staker: &str,
    token_id: &str,
) -> AnyResult<()> {
    mint_nft(app, collection_id, staker, token_id)?;
    stake_nft(app, collection_id, module, staker, token_id)?;

    Ok(())
}

pub fn unstake_nfts(
    app: &mut OmniflixApp,
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
    app: &mut OmniflixApp,
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

pub fn claim_nfts(app: &mut OmniflixApp, module: &Addr, sender: &str) -> AnyResult<AppResponse> {
    app.execute_contract(
        addr!(sender),
        module.clone(),
        &ExecuteMsg::ClaimNfts {
            token_ids: Some(vec![]),
        },
        &[],
    )
}

pub fn add_hook(
    app: &mut OmniflixApp,
    module: &Addr,
    sender: &str,
    hook: &str,
) -> AnyResult<AppResponse> {
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
    app: &mut OmniflixApp,
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
