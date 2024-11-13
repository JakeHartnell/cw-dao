use cosmwasm_schema::{cw_serde, QueryResponses};
use cw20::Cw20ReceiveMsg;
use cw_ownable::cw_ownable_execute;
use cw_vesting::msg::InstantiateMsg as PayrollInstantiateMsg;

#[cw_serde]
pub struct InstantiateMsg {
    pub owner: Option<String>,
    pub vesting_code_id: u64,
}

#[cw_ownable_execute]
#[cw_serde]
#[derive(cw_orch::ExecuteFns)]
pub enum ExecuteMsg {
    /// Instantiates a new vesting contract that is funded by a cw20 token.
    Receive(Cw20ReceiveMsg),
    /// Instantiates a new vesting contract that is funded by a native token.
    #[cw_orch(payable)]
    InstantiateNativePayrollContract {
        instantiate_msg: PayrollInstantiateMsg,
        label: String,
    },

    /// Callable only by the current owner. Updates the code ID used
    /// while instantiating vesting contracts.
    UpdateCodeId { vesting_code_id: u64 },
}

// Receiver setup
#[cw_serde]
pub enum ReceiveMsg {
    /// Funds a vesting contract with a cw20 token
    InstantiatePayrollContract {
        instantiate_msg: PayrollInstantiateMsg,
        label: String,
    },
}

#[cw_serde]
#[derive(QueryResponses, cw_orch::QueryFns)]
pub enum QueryMsg {
    /// Returns list of all vesting payment contracts
    #[returns(Vec<crate::state::VestingContract>)]
    ListVestingContracts {
        start_after: Option<String>,
        limit: Option<u32>,
    },
    /// Returns list of all vesting payment contracts in reverse
    #[returns(Vec<crate::state::VestingContract>)]
    ListVestingContractsReverse {
        start_before: Option<String>,
        limit: Option<u32>,
    },
    /// Returns list of all vesting payment contracts by who instantiated them
    #[returns(Vec<crate::state::VestingContract>)]
    ListVestingContractsByInstantiator {
        instantiator: String,
        start_after: Option<String>,
        limit: Option<u32>,
    },
    /// Returns list of all vesting payment contracts by who instantiated them in reverse
    #[returns(Vec<crate::state::VestingContract>)]
    ListVestingContractsByInstantiatorReverse {
        instantiator: String,
        start_before: Option<String>,
        limit: Option<u32>,
    },
    /// Returns list of all vesting payment contracts by recipient
    #[returns(Vec<crate::state::VestingContract>)]
    ListVestingContractsByRecipient {
        recipient: String,
        start_after: Option<String>,
        limit: Option<u32>,
    },
    /// Returns list of all vesting payment contracts by recipient in reverse
    #[returns(Vec<crate::state::VestingContract>)]
    ListVestingContractsByRecipientReverse {
        recipient: String,
        start_before: Option<String>,
        limit: Option<u32>,
    },
    /// Returns info about the contract ownership, if set
    #[returns(::cw_ownable::Ownership<::cosmwasm_std::Addr>)]
    Ownership {},

    /// Returns the code ID currently being used to instantiate vesting contracts.
    #[returns(::std::primitive::u64)]
    CodeId {},
}
