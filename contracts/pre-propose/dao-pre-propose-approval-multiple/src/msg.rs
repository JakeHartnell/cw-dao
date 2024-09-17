use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Empty;
use dao_pre_propose_base::msg::{
    ExecuteMsg as ExecuteBase, InstantiateMsg as InstantiateBase, MigrateMsg as MigrateBase,
    QueryMsg as QueryBase,
};
use dao_voting::{
    multiple_choice::{MultipleChoiceAutoVote, MultipleChoiceOptions},
    proposal::MultipleChoiceProposeMsg as ProposeMsg,
};

pub use dao_voting::approval::ApprovalExecuteExt as ExecuteExt;

#[cw_serde]
pub enum ProposeMessage {
    /// The propose message used to make a proposal to this
    /// module. Note that this is identical to the propose message
    /// used by dao-proposal-multiple, except that it omits the
    /// `proposer` field which it fills in for the sender.
    Propose {
        title: String,
        description: String,
        choices: MultipleChoiceOptions,
        vote: Option<MultipleChoiceAutoVote>,
    },
}

#[cw_serde]
pub struct InstantiateExt {
    pub approver: String,
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryExt {
    /// List the approver address
    #[returns(cosmwasm_std::Addr)]
    Approver {},
    /// Return whether or not the proposal is pending
    #[returns(bool)]
    IsPending { id: u64 },
    /// A proposal, pending or completed.
    #[returns(crate::state::Proposal)]
    Proposal { id: u64 },
    /// A pending proposal
    #[returns(crate::state::Proposal)]
    PendingProposal { id: u64 },
    /// List of proposals awaiting approval
    #[returns(Vec<crate::state::Proposal>)]
    PendingProposals {
        start_after: Option<u64>,
        limit: Option<u32>,
    },
    #[returns(Vec<crate::state::Proposal>)]
    ReversePendingProposals {
        start_before: Option<u64>,
        limit: Option<u32>,
    },
    /// A completed proposal
    #[returns(crate::state::Proposal)]
    CompletedProposal { id: u64 },
    /// List of completed proposals
    #[returns(Vec<crate::state::Proposal>)]
    CompletedProposals {
        start_after: Option<u64>,
        limit: Option<u32>,
    },
    #[returns(Vec<crate::state::Proposal>)]
    ReverseCompletedProposals {
        start_before: Option<u64>,
        limit: Option<u32>,
    },
    /// The completed approval ID for a created proposal ID.
    #[returns(::std::option::Option<u64>)]
    CompletedProposalIdForCreatedProposalId { id: u64 },
}

pub type InstantiateMsg = InstantiateBase<InstantiateExt>;
pub type ExecuteMsg = ExecuteBase<ProposeMessage, ExecuteExt>;
pub type QueryMsg = QueryBase<QueryExt>;
pub type MigrateMsg = MigrateBase<Empty>;

/// Internal version of the propose message that includes the
/// `proposer` field. The module will fill this in based on the sender
/// of the external message.
#[cw_serde]
pub(crate) enum ProposeMessageInternal {
    Propose(ProposeMsg),
}
