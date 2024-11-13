use cw_orch::{anyhow, prelude::*};

use crate::{
    tests::{ADMIN, PREFIX},
    DaoStakingSuite,
};

#[test]
fn test_staking_suite() -> anyhow::Result<()> {
    let mock = MockBech32::new(PREFIX);
    let admin = mock.addr_make(ADMIN);
    let _app = DaoStakingSuite::deploy_on(mock.clone(), admin.clone())?;
    mock.next_block().unwrap();
    Ok(())
}
