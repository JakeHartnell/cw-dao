use cosmwasm_std::{Addr, StdResult, Uint128};
use cw721_controllers::NftClaimsResponse;
use cw_controllers::HooksResponse;
use dao_interface::voting::{
    InfoResponse, TotalPowerAtHeightResponse, VotingPowerAtHeightResponse,
};
use omniflix_std::types::omniflix::onft::v1beta1::{QueryOnftRequest, QueryOnftResponse};

use crate::{msg::QueryMsg, state::Config};

use super::app::OmniflixApp;

pub fn query_config(app: &OmniflixApp, module: &Addr) -> StdResult<Config> {
    let config = app.wrap().query_wasm_smart(module, &QueryMsg::Config {})?;
    Ok(config)
}

pub fn query_claims(app: &OmniflixApp, module: &Addr, addr: &str) -> StdResult<NftClaimsResponse> {
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

pub fn query_hooks(app: &OmniflixApp, module: &Addr) -> StdResult<HooksResponse> {
    let hooks = app.wrap().query_wasm_smart(module, &QueryMsg::Hooks {})?;
    Ok(hooks)
}

pub fn query_staked_nfts(
    app: &OmniflixApp,
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
    app: &OmniflixApp,
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
    app: &OmniflixApp,
    module: &Addr,
    height: Option<u64>,
) -> StdResult<TotalPowerAtHeightResponse> {
    let power = app
        .wrap()
        .query_wasm_smart(module, &QueryMsg::TotalPowerAtHeight { height })?;
    Ok(power)
}

pub fn query_dao(app: &OmniflixApp, module: &Addr) -> StdResult<Addr> {
    let dao = app.wrap().query_wasm_smart(module, &QueryMsg::Dao {})?;
    Ok(dao)
}

pub fn query_info(app: &OmniflixApp, module: &Addr) -> StdResult<InfoResponse> {
    let info = app.wrap().query_wasm_smart(module, &QueryMsg::Info {})?;
    Ok(info)
}

pub fn query_total_and_voting_power(
    app: &OmniflixApp,
    module: &Addr,
    addr: &str,
    height: Option<u64>,
) -> StdResult<(Uint128, Uint128)> {
    let total_power = query_total_power(app, module, height)?;
    let voting_power = query_voting_power(app, module, addr, height)?;

    Ok((total_power.power, voting_power.power))
}

pub fn query_nft_owner(
    app: &OmniflixApp,
    collection_id: &str,
    token_id: &str,
) -> StdResult<String> {
    let response: QueryOnftResponse = app
        .wrap()
        .query(
            &QueryOnftRequest {
                denom_id: collection_id.to_string(),
                id: token_id.to_string(),
            }
            .into(),
        )
        .unwrap();
    Ok(response.onft.unwrap().owner)
}
