use cosmwasm_schema::cw_serde;
use cosmwasm_std::Addr;

use crate::deposit::CheckedDepositInfo;

#[cw_serde]
pub enum ApproverProposeMessage {
    Propose {
        title: String,
        description: String,
        approval_id: u64,
    },
}

#[cw_serde]
pub enum ApprovalExecuteExt {
    /// Approve a proposal, only callable by approver
    Approve { id: u64 },
    /// Reject a proposal, only callable by approver
    Reject { id: u64 },
    /// Updates the approver, can only be called the current approver
    UpdateApprover { address: String },
}

#[cw_serde]
pub enum ApprovalProposalStatus {
    /// The proposal is pending approval.
    Pending {},
    /// The proposal has been approved.
    Approved {
        /// The created proposal ID.
        created_proposal_id: u64,
    },
    /// The proposal has been rejected.
    Rejected {},
}

#[cw_serde]
pub struct ApprovalProposal<ProposeMsg> {
    /// The status of a completed proposal.
    pub status: ApprovalProposalStatus,
    /// The approval ID used to identify this pending proposal.
    pub approval_id: u64,
    /// The address that created the proposal.
    pub proposer: Addr,
    /// The propose message that ought to be executed on the proposal
    /// message if this proposal is approved.
    pub msg: ProposeMsg,
    /// Snapshot of the deposit info at the time of proposal
    /// submission.
    pub deposit: Option<CheckedDepositInfo>,
}
