use cosmwasm_schema::{cw_serde, QueryResponses};
use dao_cw721_extensions::roles::MetadataExt;
use dao_dao_macros::voting_module_query;

#[cw_serde]
pub struct NftMintMsg {
    /// Unique ID of the NFT
    pub token_id: String,
    /// The owner of the newly minter NFT
    pub owner: String,
    /// Universal resource identifier for this NFT
    /// Should point to a JSON file that conforms to the ERC721
    /// Metadata JSON Schema
    pub token_uri: Option<String>,
    /// Any custom extension used by this contract
    pub extension: MetadataExt,
}

#[cw_serde]
pub enum NftContract {
    Existing {
        /// Address of an already instantiated cw721-weighted-roles token contract.
        address: String,
    },
    New {
        /// Code ID for cw721 token contract.
        code_id: u64,
        /// Label to use for instantiated cw721 contract.
        label: String,
        /// NFT collection name
        name: String,
        /// NFT collection symbol
        symbol: String,
        /// Initial NFTs to mint when instantiating the new cw721 contract.
        /// If empty, an error is thrown.
        initial_nfts: Vec<NftMintMsg>,
    },
}

#[cw_serde]
pub struct InstantiateMsg {
    /// Info about the associated NFT contract
    pub nft_contract: NftContract,
}

#[cw_serde]
#[derive(cw_orch::ExecuteFns)]
pub enum ExecuteMsg {}

#[voting_module_query]
#[cw_serde]
#[derive(QueryResponses, cw_orch::QueryFns)]
pub enum QueryMsg {
    #[returns(crate::state::Config)]
    Config {},
}
