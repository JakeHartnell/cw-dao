use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Binary;
use cw721_base::InstantiateMsg as Cw721InstantiateMsg;
use dao_interface::token::NewTokenInfo;

#[cw_serde]
pub struct InstantiateMsg {}

#[cw_serde]
#[derive(cw_orch::ExecuteFns)]
pub enum ExecuteMsg {
    /// Example NFT factory implementation
    NftFactory {
        code_id: u64,
        cw721_instantiate_msg: Cw721InstantiateMsg,
        initial_nfts: Vec<Binary>,
    },
    /// Example NFT factory implentation that execpts funds
    NftFactoryWithFunds {
        code_id: u64,
        cw721_instantiate_msg: Cw721InstantiateMsg,
        initial_nfts: Vec<Binary>,
    },
    /// Used for testing no callback
    NftFactoryNoCallback {},
    /// Used for testing wrong callback
    NftFactoryWrongCallback {},
    /// Example Factory Implementation
    TokenFactoryFactory(NewTokenInfo),
    /// Example Factory Implementation that accepts funds
    TokenFactoryFactoryWithFunds(NewTokenInfo),
    /// Used for testing no callback
    TokenFactoryFactoryNoCallback {},
    /// Used for testing wrong callback
    TokenFactoryFactoryWrongCallback {},
    /// Validate NFT DAO
    ValidateNftDao {},
}

#[cw_serde]
#[derive(QueryResponses, cw_orch::QueryFns)]
pub enum QueryMsg {
    #[returns(dao_interface::voting::InfoResponse)]
    Info {},
}
