use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{CosmosMsg, Empty};
use dao_pre_propose_base::msg::{
    ExecuteMsg as ExecuteBase, InstantiateMsg as InstantiateBase, MigrateMsg as MigrateBase,
    QueryMsg as QueryBase,
};
use dao_voting::approval::ApproverProposeMessage;

#[cw_serde]
pub struct InstantiateMsg {
    pub pre_propose_approval_contract: String,
}

#[cw_serde]
pub enum ExecuteExt {
    // Reset approver back to DAO that set up this approver contract. Only
    // callable by the DAO.
    ResetApprover {},
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryExt {
    #[returns(cosmwasm_std::Addr)]
    PreProposeApprovalContract {},
    #[returns(::std::option::Option<u64>)]
    PreProposeApprovalIdForApproverProposalId { id: u64 },
    #[returns(::std::option::Option<u64>)]
    ApproverProposalIdForPreProposeApprovalId { id: u64 },
}

pub type BaseInstantiateMsg = InstantiateBase<Empty>;
pub type ExecuteMsg = ExecuteBase<ApproverProposeMessage, ExecuteExt>;
pub type QueryMsg = QueryBase<QueryExt>;
pub type MigrateMsg = MigrateBase<Empty>;

/// Internal version of the propose message that includes the
/// `proposer` field. The module will fill this in based on the sender
/// of the external message.
#[cw_serde]
pub enum ProposeMessageInternal {
    Propose {
        title: String,
        description: String,
        msgs: Vec<CosmosMsg<Empty>>,
        proposer: Option<String>,
    },
}
