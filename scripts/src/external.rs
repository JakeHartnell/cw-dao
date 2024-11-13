use cw_orch::prelude::*;
use dao_cw_orch::*;

// external suite
pub struct DaoExternalSuite<Chain> {
    pub admin_factory: DaoExternalAdminFactory<Chain>,
    pub btsg_ft_factory: DaoExternalFantokenFactory<Chain>,
    pub payroll_factory: DaoExternalPayrollFactory<Chain>,
    pub cw_tokenswap: DaoExternalTokenSwap<Chain>,
    pub cw_tokenfactory_issuer: DaoExternalTokenfactoryIssuer<Chain>,
    pub cw_vesting: DaoExternalCwVesting<Chain>,
    pub cw721_roles: DaoExternalCw721Roles<Chain>,
    pub migrator: DaoExternalMigrator<Chain>,
}

impl<Chain: CwEnv> DaoExternalSuite<Chain> {
    pub fn new(chain: Chain) -> DaoExternalSuite<Chain> {
        DaoExternalSuite::<Chain> {
            admin_factory: DaoExternalAdminFactory::new("cw_admin_factory", chain.clone()),
            btsg_ft_factory: DaoExternalFantokenFactory::new("btsg_ft_factory", chain.clone()),
            payroll_factory: DaoExternalPayrollFactory::new("cw_payroll", chain.clone()),
            cw_tokenswap: DaoExternalTokenSwap::new("cw_tokenswap", chain.clone()),
            cw_tokenfactory_issuer: DaoExternalTokenfactoryIssuer::new(
                "cw_tokenfactory",
                chain.clone(),
            ),
            cw_vesting: DaoExternalCwVesting::new("cw_vesting", chain.clone()),
            cw721_roles: DaoExternalCw721Roles::new("cw721_roles", chain.clone()),
            migrator: DaoExternalMigrator::new("dao_migrator", chain.clone()),
        }
    }

    pub fn upload(&self) -> Result<(), CwOrchError> {
        self.admin_factory.upload()?;
        self.btsg_ft_factory.upload()?;
        self.payroll_factory.upload()?;
        self.cw_tokenswap.upload()?;
        self.cw_tokenfactory_issuer.upload()?;
        self.cw_vesting.upload()?;
        self.cw721_roles.upload()?;
        self.migrator.upload()?;

        Ok(())
    }
}