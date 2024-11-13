use cw_orch::prelude::*;
use dao_cw_orch::DaoDaoCore;

use crate::{
    DaoDistributionSuite, DaoExternalSuite, DaoProposalSuite, DaoStakingSuite, DaoVotingSuite,
};

// full dao suite
pub struct DaoDao<Chain> {
    pub dao_core: DaoDaoCore<Chain>,
    pub proposal_suite: DaoProposalSuite<Chain>,
    pub voting_suite: DaoVotingSuite<Chain>,
    pub staking_suite: DaoStakingSuite<Chain>,
    pub distribution_suite: DaoDistributionSuite<Chain>,
    pub external_suite: DaoExternalSuite<Chain>,
}

impl<Chain: CwEnv> DaoDao<Chain> {
    pub fn new(chain: Chain) -> DaoDao<Chain> {
        DaoDao::<Chain> {
            dao_core: DaoDaoCore::new("dao_dao_core", chain.clone()),
            proposal_suite: DaoProposalSuite::new(chain.clone()),
            voting_suite: DaoVotingSuite::new(chain.clone()),
            staking_suite: DaoStakingSuite::new(chain.clone()),
            distribution_suite: DaoDistributionSuite::new(chain.clone()),
            external_suite: DaoExternalSuite::new(chain.clone()),
        }
    }

    pub fn upload(&self) -> Result<(), CwOrchError> {
        self.dao_core.upload()?;
        self.proposal_suite.upload()?;
        self.voting_suite.upload()?;
        self.staking_suite.upload()?;
        self.distribution_suite.upload()?;
        self.external_suite.upload()?;
        Ok(())
    }
}
