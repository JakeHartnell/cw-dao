use crate::external::*;
use cw_orch::prelude::*;

// admin factory
impl<Chain: CwEnv> cw_orch::contract::Deploy<Chain> for DaoExternalSuite<Chain> {
    // We don't have a custom error type
    type Error = CwOrchError;
    type DeployData = Addr;

    fn store_on(chain: Chain) -> Result<Self, Self::Error> {
        let suite = DaoExternalSuite::new(chain.clone());
        suite.upload()?;
        Ok(suite)
    }

    fn deployed_state_file_path() -> Option<String> {
        None
    }

    fn get_contracts_mut(&mut self) -> Vec<Box<&mut dyn ContractInstance<Chain>>> {
        vec![
            Box::new(&mut self.admin_factory),
            Box::new(&mut self.btsg_ft_factory),
            Box::new(&mut self.payroll_factory),
            Box::new(&mut self.cw_tokenswap),
            Box::new(&mut self.cw_tokenfactory_issuer),
            Box::new(&mut self.cw_vesting),
            Box::new(&mut self.cw721_roles),
            Box::new(&mut self.migrator),
        ]
    }

    fn load_from(chain: Chain) -> Result<Self, Self::Error> {
        let suite = Self::new(chain.clone());
        Ok(suite)
    }

    fn deploy_on(chain: Chain, _data: Self::DeployData) -> Result<Self, Self::Error> {
        // ########### Upload ##############
        let suite: DaoExternalSuite<Chain> = DaoExternalSuite::store_on(chain.clone())?;
        Ok(suite)
    }
}
