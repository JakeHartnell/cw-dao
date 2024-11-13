use cw_orch::prelude::*;
use dao_cw_orch::*;

pub struct DaoPreProposeSuite<Chain> {
    pub pre_prop_approval_single: DaoPreProposeApprovalSingle<Chain>,
    pub pre_prop_approver: DaoPreProposeApprover<Chain>,
    pub pre_prop_multiple: DaoPreProposeMultiple<Chain>,
    pub pre_prop_single: DaoPreProposeSingle<Chain>,
}

impl<Chain: CwEnv> DaoPreProposeSuite<Chain> {
    pub fn new(chain: Chain) -> DaoPreProposeSuite<Chain> {
        DaoPreProposeSuite::<Chain> {
            pre_prop_approval_single: DaoPreProposeApprovalSingle::new(
                "dao_pre_propose_approval_single",
                chain.clone(),
            ),
            pre_prop_approver: DaoPreProposeApprover::new(
                "dao_pre_propose_approver",
                chain.clone(),
            ),
            pre_prop_multiple: DaoPreProposeMultiple::new(
                "dao_pre_propose_multiple",
                chain.clone(),
            ),
            pre_prop_single: DaoPreProposeSingle::new("dao_pre_propose_single", chain.clone()),
        }
    }

    pub fn upload(&self) -> Result<(), CwOrchError> {
        self.pre_prop_approval_single.upload()?;
        self.pre_prop_approver.upload()?;
        self.pre_prop_multiple.upload()?;
        self.pre_prop_single.upload()?;

        Ok(())
    }
}

// proposal suite
pub struct DaoProposalSuite<Chain> {
    pub prop_single: DaoProposalSingle<Chain>,
    pub prop_multiple: DaoProposalMultiple<Chain>,
    pub prop_condocert: DaoProposalCondorcet<Chain>,
    pub prop_sudo: DaoProposalSudo<Chain>,
    pub pre_prop_suite: DaoPreProposeSuite<Chain>,
}

impl<Chain: CwEnv> DaoProposalSuite<Chain> {
    pub fn new(chain: Chain) -> DaoProposalSuite<Chain> {
        DaoProposalSuite::<Chain> {
            prop_single: DaoProposalSingle::new("dao_proposal_single", chain.clone()),
            prop_multiple: DaoProposalMultiple::new("dao_proposal_multiple", chain.clone()),
            prop_condocert: DaoProposalCondorcet::new("dao_proposal_condocert", chain.clone()),
            prop_sudo: DaoProposalSudo::new("dao_proposal_sudo", chain.clone()),
            pre_prop_suite: DaoPreProposeSuite::new(chain.clone()),
        }
    }

    pub fn upload(&self) -> Result<(), CwOrchError> {
        self.prop_single.upload()?;
        self.prop_multiple.upload()?;
        self.prop_condocert.upload()?;
        self.prop_sudo.upload()?;
        self.pre_prop_suite.upload()?;
        Ok(())
    }
}
