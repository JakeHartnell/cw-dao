use cw_orch::{anyhow, prelude::*};

use crate::{
    distribution::DaoDistributionSuite,
    tests::{ADMIN, PREFIX},
};

#[test]
fn test_distribution_suite() -> anyhow::Result<()> {
    let mock = MockBech32::new(PREFIX);
    let admin = mock.addr_make(ADMIN);
    let _app = DaoDistributionSuite::deploy_on(mock.clone(), admin.clone())?;
    mock.next_block().unwrap();
    Ok(())
}
