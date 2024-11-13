use crate::propose::*;
use cw_orch::prelude::*;

// pre-proposal suite
impl<Chain: CwEnv> cw_orch::contract::Deploy<Chain> for DaoPreProposeSuite<Chain> {
    // We don't have a custom error type
    type Error = CwOrchError;
    type DeployData = Addr;

    fn store_on(chain: Chain) -> Result<Self, Self::Error> {
        let suite = DaoPreProposeSuite::new(chain.clone());
        suite.upload()?;
        Ok(suite)
    }

    fn deployed_state_file_path() -> Option<String> {
        None
    }

    fn get_contracts_mut(&mut self) -> Vec<Box<&mut dyn ContractInstance<Chain>>> {
        vec![
            Box::new(&mut self.pre_prop_approval_single),
            Box::new(&mut self.pre_prop_approver),
            Box::new(&mut self.pre_prop_multiple),
            Box::new(&mut self.pre_prop_single),
        ]
    }

    fn load_from(chain: Chain) -> Result<Self, Self::Error> {
        let factory = Self::new(chain.clone());
        Ok(factory)
    }

    fn deploy_on(chain: Chain, _data: Self::DeployData) -> Result<Self, Self::Error> {
        // ########### Upload ##############
        let suite: DaoPreProposeSuite<Chain> = DaoPreProposeSuite::store_on(chain.clone())?;
        Ok(suite)
    }
}

// proposal suite
impl<Chain: CwEnv> cw_orch::contract::Deploy<Chain> for DaoProposalSuite<Chain> {
    // We don't have a custom error type
    type Error = CwOrchError;
    type DeployData = Addr;

    fn store_on(chain: Chain) -> Result<Self, Self::Error> {
        let suite = DaoProposalSuite::new(chain.clone());
        suite.upload()?;
        Ok(suite)
    }

    fn deployed_state_file_path() -> Option<String> {
        None
    }

    fn get_contracts_mut(&mut self) -> Vec<Box<&mut dyn ContractInstance<Chain>>> {
        let mut boxs = vec![];
        let prop: Vec<Box<&mut dyn ContractInstance<Chain>>> = vec![
            Box::new(&mut self.prop_single),
            Box::new(&mut self.prop_multiple),
            Box::new(&mut self.prop_condocert),
        ];

        boxs.extend(prop);
        boxs.extend(self.pre_prop_suite.get_contracts_mut());
        boxs
    }

    fn load_from(chain: Chain) -> Result<Self, Self::Error> {
        let factory = Self::new(chain.clone());
        Ok(factory)
    }

    fn deploy_on(chain: Chain, _data: Self::DeployData) -> Result<Self, Self::Error> {
        // ########### Upload ##############
        let suite: DaoProposalSuite<Chain> = DaoProposalSuite::store_on(chain.clone())?;
        Ok(suite)
    }
}
