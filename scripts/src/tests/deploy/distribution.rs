use crate::DaoDistributionSuite;
use cw_orch::prelude::*;

// distribution suite
impl<Chain: CwEnv> cw_orch::contract::Deploy<Chain> for DaoDistributionSuite<Chain> {
    // We don't have a custom error type
    type Error = CwOrchError;
    type DeployData = Addr;

    fn store_on(chain: Chain) -> Result<Self, Self::Error> {
        let suite = DaoDistributionSuite::new(chain.clone());
        suite.upload()?;
        Ok(suite)
    }

    fn deployed_state_file_path() -> Option<String> {
        None
    }

    fn get_contracts_mut(&mut self) -> Vec<Box<&mut dyn ContractInstance<Chain>>> {
        vec![
            Box::new(&mut self.fund_distr),
            Box::new(&mut self.reward_distr),
        ]
    }

    fn load_from(chain: Chain) -> Result<Self, Self::Error> {
        let suite = Self::new(chain.clone());
        Ok(suite)
    }

    fn deploy_on(chain: Chain, _data: Self::DeployData) -> Result<Self, Self::Error> {
        // ########### Upload ##############
        let suite: DaoDistributionSuite<Chain> = DaoDistributionSuite::store_on(chain.clone())?;
        Ok(suite)
    }
}
