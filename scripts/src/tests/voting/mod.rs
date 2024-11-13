use cw_orch::{anyhow, prelude::*};

use crate::{
    tests::{ADMIN, PREFIX},
    DaoVotingSuite,
};

#[test]
fn test_voting_suite() -> anyhow::Result<()> {
    let mock = MockBech32::new(PREFIX);
    let admin = mock.addr_make(ADMIN);
    let _app = DaoVotingSuite::deploy_on(mock.clone(), admin.clone())?;
    mock.next_block().unwrap();
    Ok(())
}
