use crate::staking::*;
use cw_orch::prelude::*;

// staking suite
impl<Chain: CwEnv> cw_orch::contract::Deploy<Chain> for DaoStakingSuite<Chain> {
    // We don't have a custom error type
    type Error = CwOrchError;
    type DeployData = Addr;

    fn store_on(chain: Chain) -> Result<Self, Self::Error> {
        let suite = DaoStakingSuite::new(chain.clone());
        suite.upload()?;
        Ok(suite)
    }

    fn deployed_state_file_path() -> Option<String> {
        None
    }

    fn get_contracts_mut(&mut self) -> Vec<Box<&mut dyn ContractInstance<Chain>>> {
        vec![
            Box::new(&mut self.cw20_stake),
            Box::new(&mut self.exteral_rewards),
            Box::new(&mut self.rewards_distributor),
        ]
    }

    fn load_from(chain: Chain) -> Result<Self, Self::Error> {
        let factory = Self::new(chain.clone());
        Ok(factory)
    }

    fn deploy_on(chain: Chain, _data: Self::DeployData) -> Result<Self, Self::Error> {
        // ########### Upload ##############
        let suite: DaoStakingSuite<Chain> = DaoStakingSuite::store_on(chain.clone())?;
        Ok(suite)
    }
}
