use cw_orch::{anyhow, prelude::*};

use crate::{
    propose::{DaoPreProposeSuite, DaoProposalSuite},
    tests::{ADMIN, PREFIX},
};

#[test]
fn test_proposals_suite() -> anyhow::Result<()> {
    let mock = MockBech32::new(PREFIX);
    let admin = mock.addr_make(ADMIN);
    let _pre_prop_suite = DaoPreProposeSuite::deploy_on(mock.clone(), admin.clone())?;
    let _props = DaoProposalSuite::deploy_on(mock.clone(), admin.clone())?;
    mock.next_block().unwrap();
    Ok(())
}
