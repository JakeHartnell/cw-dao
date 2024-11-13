use crate::DaoDao;
use cw_orch::prelude::*;
// distribution suite
impl<Chain: CwEnv> cw_orch::contract::Deploy<Chain> for DaoDao<Chain> {
    // We don't have a custom error type
    type Error = CwOrchError;
    type DeployData = Addr;

    fn store_on(chain: Chain) -> Result<Self, Self::Error> {
        let suite = DaoDao::new(chain.clone());
        suite.upload()?;
        Ok(suite)
    }

    fn deployed_state_file_path() -> Option<String> {
        None
    }

    fn get_contracts_mut(&mut self) -> Vec<Box<&mut dyn ContractInstance<Chain>>> {
        let mut cs: Vec<Box<&mut dyn ContractInstance<Chain>>> = vec![];
        let res: Vec<Box<&mut dyn ContractInstance<Chain>>> = vec![Box::new(&mut self.dao_core)];

        cs.extend(res);
        cs.extend(self.distribution_suite.get_contracts_mut());
        cs.extend(self.proposal_suite.get_contracts_mut());
        cs.extend(self.staking_suite.get_contracts_mut());
        cs.extend(self.voting_suite.get_contracts_mut());
        cs.extend(self.external_suite.get_contracts_mut());
        cs
    }

    fn load_from(chain: Chain) -> Result<Self, Self::Error> {
        let suite = Self::new(chain.clone());
        Ok(suite)
    }

    fn deploy_on(chain: Chain, _data: Self::DeployData) -> Result<Self, Self::Error> {
        // ########### Upload ##############
        let suite: DaoDao<Chain> = DaoDao::store_on(chain.clone())?;
        Ok(suite)
    }
}
