use cw_orch::{anyhow, prelude::*};
use crate::DaoExternalSuite;
use super::{ADMIN, PREFIX};

pub mod admin_factory;
pub mod btsg_ft_factory;
pub mod cw721_roles;
pub mod dao_migration;
pub mod payroll;
pub mod token_swap;
pub mod tokenfactory_issuer;
pub mod vesting;

#[test]
fn test_external_suite() -> anyhow::Result<()> {
    let mock = MockBech32::new(PREFIX);
    let admin = mock.addr_make(ADMIN);
    let _app = DaoExternalSuite::deploy_on(mock.clone(), admin.clone())?;
    mock.next_block().unwrap();
    Ok(())
}
