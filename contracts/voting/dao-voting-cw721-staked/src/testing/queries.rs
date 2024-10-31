use cosmwasm_std::{Addr, StdResult, Uint128};
use cw721_controllers::NftClaimsResponse;
use cw_controllers::HooksResponse;
use cw_multi_test::App;
use dao_interface::voting::{
    InfoResponse, TotalPowerAtHeightResponse, VotingPowerAtHeightResponse,
};

use crate::{msg::QueryMsg, state::Config};

pub fn query_config(app: &App, module: &Addr) -> StdResult<Config> {
    let config = app.wrap().query_wasm_smart(module, &QueryMsg::Config {})?;
    Ok(config)
}

pub fn query_claims(app: &App, module: &Addr, addr: &str) -> StdResult<NftClaimsResponse> {
    let claims = app.wrap().query_wasm_smart(
        module,
        &QueryMsg::NftClaims {
            address: addr.to_string(),
            start_after: None,
            limit: None,
        },
    )?;
    Ok(claims)
}

pub fn query_hooks(app: &App, module: &Addr) -> StdResult<HooksResponse> {
    let hooks = app.wrap().query_wasm_smart(module, &QueryMsg::Hooks {})?;
    Ok(hooks)
}

pub fn query_staked_nfts(
    app: &App,
    module: &Addr,
    addr: &str,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<Vec<String>> {
    let nfts = app.wrap().query_wasm_smart(
        module,
        &QueryMsg::StakedNfts {
            address: addr.to_string(),
            start_after,
            limit,
        },
    )?;
    Ok(nfts)
}

pub fn query_voting_power(
    app: &App,
    module: &Addr,
    addr: &str,
    height: Option<u64>,
) -> StdResult<VotingPowerAtHeightResponse> {
    let power = app.wrap().query_wasm_smart(
        module,
        &QueryMsg::VotingPowerAtHeight {
            address: addr.to_string(),
            height,
        },
    )?;
    Ok(power)
}

pub fn query_total_power(
    app: &App,
    module: &Addr,
    height: Option<u64>,
) -> StdResult<TotalPowerAtHeightResponse> {
    let power = app
        .wrap()
        .query_wasm_smart(module, &QueryMsg::TotalPowerAtHeight { height })?;
    Ok(power)
}

pub fn query_info(app: &App, module: &Addr) -> StdResult<InfoResponse> {
    let info = app.wrap().query_wasm_smart(module, &QueryMsg::Info {})?;
    Ok(info)
}

pub fn query_total_and_voting_power(
    app: &App,
    module: &Addr,
    addr: &str,
    height: Option<u64>,
) -> StdResult<(Uint128, Uint128)> {
    let total_power = query_total_power(app, module, height)?;
    let voting_power = query_voting_power(app, module, addr, height)?;

    Ok((total_power.power, voting_power.power))
}

pub fn query_nft_owner(app: &App, nft: &Addr, token_id: &str) -> StdResult<cw721::OwnerOfResponse> {
    let owner = app.wrap().query_wasm_smart(
        nft,
        &cw721::Cw721QueryMsg::OwnerOf {
            token_id: token_id.to_string(),
            include_expired: None,
        },
    )?;
    Ok(owner)
}
