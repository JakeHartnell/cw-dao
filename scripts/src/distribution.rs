use cw_orch::prelude::*;
use dao_cw_orch::*;

// cw-funds-distributor
pub struct DaoDistributionSuite<Chain> {
    pub fund_distr: DaoFundsDistributor<Chain>,
    pub reward_distr: DaoRewardsDistributor<Chain>,
}

impl<Chain: CwEnv> DaoDistributionSuite<Chain> {
    pub fn new(chain: Chain) -> DaoDistributionSuite<Chain> {
        DaoDistributionSuite::<Chain> {
            fund_distr: DaoFundsDistributor::new("cw_funds_distributor", chain.clone()),
            reward_distr: DaoRewardsDistributor::new("dao_rewards_distributor", chain.clone()),
        }
    }

    pub fn upload(&self) -> Result<(), CwOrchError> {
        self.fund_distr.upload()?;
        self.reward_distr.upload()?;
        Ok(())
    }
}
