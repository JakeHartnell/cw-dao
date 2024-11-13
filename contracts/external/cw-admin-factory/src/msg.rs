use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Binary};

#[cw_serde]
pub struct InstantiateMsg {
    /// The account allowed to execute this contract. If no admin, anyone can
    /// execute it.
    pub admin: Option<String>,
}

#[cw_serde]
#[derive(cw_orch::ExecuteFns)]
pub enum ExecuteMsg {
    /// Instantiates the target contract with the provided instantiate message,
    /// code ID, and label and updates the contract's admin to be itself.
    #[cw_orch(payable)]
    InstantiateContractWithSelfAdmin {
        instantiate_msg: Binary,
        code_id: u64,
        label: String,
    },
    /// Instantiates the target contract with the provided instantiate message,
    /// code ID, label, and salt, via instantiate2 to give a predictable
    /// address, and updates the contract's admin to be itself.
    #[cw_orch(payable)]
    Instantiate2ContractWithSelfAdmin {
        instantiate_msg: Binary,
        code_id: u64,
        label: String,
        salt: Binary,
        /// Optionally specify the expected address and fail if it doesn't match
        /// the instantiated contract. This makes it easy for a consumer to
        /// validate that they are using the correct address elsewhere.
        expect: Option<String>,
    },
}

#[cw_serde]
#[derive(QueryResponses, cw_orch::QueryFns)]
pub enum QueryMsg {
    #[returns(AdminResponse)]
    Admin {},
}

#[cw_serde]
pub struct MigrateMsg {}

#[cw_serde]
pub struct AdminResponse {
    pub admin: Option<Addr>,
}
