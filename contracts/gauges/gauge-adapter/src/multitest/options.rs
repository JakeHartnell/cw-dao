use cosmwasm_std::{coin, Addr};

use crate::multitest::suite::SuiteBuilder;

#[test]
fn option_queries() {
    let mut suite = SuiteBuilder::new()
        .with_community_pool("community_pool")
        .with_funds("owner", &[coin(100_000, "juno")])
        .with_funds("einstein", &[coin(100_000, "juno")])
        .with_native_deposit(1_000)
        .build();

    let recipient = "user".to_owned();

    let options = suite.query_all_options().unwrap();
    // account for a default option
    assert_eq!(options.len(), 1);

    // Valid submission.
    _ = suite
        .execute_create_submission(
            suite.owner.clone(),
            "WYNDers".to_owned(),
            "https://www.wynddao.com/".to_owned(),
            recipient.clone(),
            &[coin(1_000, "juno")],
        )
        .unwrap();

    // Valid submission.
    suite
        .execute_create_submission(
            Addr::unchecked("einstein"),
            "MIBers".to_owned(),
            "https://www.mib.tech/".to_owned(),
            "einstein".to_owned(),
            &[coin(1_000, "juno")],
        )
        .unwrap();

    let options = suite.query_all_options().unwrap();
    assert_eq!(options, vec!["community_pool", "einstein", &recipient],);

    let option = suite.query_check_option("einstein".to_owned()).unwrap();
    assert!(option);

    let option = suite.query_check_option("newton".to_owned()).unwrap();
    assert!(!option);
}
