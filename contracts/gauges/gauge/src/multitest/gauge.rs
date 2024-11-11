use cosmwasm_std::{Addr, Decimal, Uint128};
use dao_voting::voting::Vote;

use super::suite::{Suite, SuiteBuilder};

use crate::error::ContractError;
use crate::msg::{GaugeMigrationConfig, GaugeResponse};

const EPOCH: u64 = 7 * 86_400;

#[test]
fn create_gauge() {
    let voter1 = "voter1";
    let voter2 = "voter2";
    let mut suite = SuiteBuilder::new()
        .with_voting_members(&[(voter1, 100), (voter2, 100)])
        .build();

    let gauge_contract = init_gauge(&mut suite, &[voter1, voter2]);

    let gauge_adapter = suite
        .instantiate_adapter_and_create_gauge(
            gauge_contract.clone(),
            &[voter1, voter2],
            (1000, "ujuno"),
            None,
            None,
            None,
        )
        .unwrap();

    let response = suite.query_gauge(gauge_contract, 0).unwrap();
    assert_eq!(
        response,
        GaugeResponse {
            id: 0,
            title: "gauge".to_owned(),
            adapter: gauge_adapter.to_string(),
            epoch_size: EPOCH,
            min_percent_selected: Some(Decimal::percent(5)),
            max_options_selected: 10,
            max_available_percentage: None,
            is_stopped: false,
            next_epoch: suite.current_time() + 7 * 86400,
            reset: None,
            total_epochs: None
        }
    );
}

#[test]
fn gauge_can_upgrade_from_self() {
    let voter1 = "voter1";
    let mut suite = SuiteBuilder::new()
        .with_voting_members(&[(voter1, 100)])
        .build();

    let gauge_contract = init_gauge(&mut suite, &[voter1]);

    let gauge_adapter = suite
        .instantiate_adapter_and_create_gauge(
            gauge_contract.clone(),
            &["option1", "option2"],
            (1000, "ujuno"),
            None,
            None,
            None,
        )
        .unwrap();

    // now let's migrate the gauge and make sure nothing breaks
    suite.auto_migrate_gauge(&gauge_contract, None).unwrap();

    let response = suite.query_gauge(gauge_contract, 0).unwrap();
    assert_eq!(
        response,
        GaugeResponse {
            id: 0,
            title: "gauge".to_owned(),
            adapter: gauge_adapter.to_string(),
            epoch_size: EPOCH,
            min_percent_selected: Some(Decimal::percent(5)),
            max_options_selected: 10,
            max_available_percentage: None,
            is_stopped: false,
            next_epoch: suite.current_time() + 7 * 86400,
            reset: None,
            total_epochs: None
        }
    );
}

#[test]
fn gauge_migrate_with_next_epochs() {
    let voter1 = "voter1";
    let mut suite = SuiteBuilder::new()
        .with_voting_members(&[(voter1, 100)])
        .build();

    let gauge_contract = init_gauge(&mut suite, &[voter1]);

    let gauge_adapter = suite
        .instantiate_adapter_and_create_gauge(
            gauge_contract.clone(),
            &["option1", "option2"],
            (1000, "ujuno"),
            None,
            None,
            None,
        )
        .unwrap();

    // previous settings
    let response = suite.query_gauge(gauge_contract.clone(), 0).unwrap();
    assert_eq!(
        response,
        GaugeResponse {
            id: 0,
            title: "gauge".to_owned(),
            adapter: gauge_adapter.to_string(),
            epoch_size: EPOCH,
            min_percent_selected: Some(Decimal::percent(5)),
            max_options_selected: 10,
            max_available_percentage: None,
            is_stopped: false,
            next_epoch: suite.current_time() + 7 * 86400,
            reset: None,
            total_epochs: None
        }
    );

    // now let's migrate the gauge and make sure nothing breaks
    let gauge_id = 0;
    // change next epoch from 7 to 14 days
    suite
        .auto_migrate_gauge(
            &gauge_contract,
            vec![(
                gauge_id,
                GaugeMigrationConfig {
                    next_epoch: Some(suite.current_time() + 14 * 86400),
                    reset: None,
                },
            )],
        )
        .unwrap();

    let response = suite.query_gauge(gauge_contract.clone(), 0).unwrap();
    assert_eq!(
        response,
        GaugeResponse {
            id: 0,
            title: "gauge".to_owned(),
            adapter: gauge_adapter.to_string(),
            epoch_size: EPOCH,
            min_percent_selected: Some(Decimal::percent(5)),
            max_options_selected: 10,
            max_available_percentage: None,
            is_stopped: false,
            next_epoch: suite.current_time() + 14 * 86400,
            reset: None,
            total_epochs: None
        }
    );

    // try to migrate updating next epoch on nonexisting gauge_id
    // actually generic error makes it more difficult to debug in presentable form, I think this is
    // enough
    let _err = suite
        .auto_migrate_gauge(
            &gauge_contract,
            vec![(
                420,
                GaugeMigrationConfig {
                    next_epoch: Some(suite.current_time() + 14 * 86400),
                    reset: None,
                },
            )],
        )
        .unwrap_err();
}

/// attach adaptor in instantiate
#[test]
fn execute_gauge() {
    let voter1 = "voter1";
    let voter2 = "voter2";
    let reward_to_distribute = (1000, "ujuno");
    let mut suite = SuiteBuilder::new()
        .with_voting_members(&[(voter1, 100), (voter2, 100)])
        .with_core_balance(reward_to_distribute)
        .build();

    suite.next_block();
    let gauge_config = suite
        .instantiate_adapter_and_return_config(
            &[voter1, voter2],
            reward_to_distribute,
            None,
            None,
            None,
        )
        .unwrap();
    suite
        .propose_update_proposal_module(voter1.to_string(), vec![gauge_config])
        .unwrap();

    suite.next_block();
    let proposal = suite.list_proposals().unwrap()[0];
    suite
        .place_vote_single(voter1, proposal, Vote::Yes)
        .unwrap();
    suite
        .place_vote_single(voter2, proposal, Vote::Yes)
        .unwrap();

    suite.next_block();
    suite
        .execute_single_proposal(voter1.to_string(), proposal)
        .unwrap();
    let proposal_modules = suite.query_proposal_modules().unwrap();
    let gauge_contract = proposal_modules[1].clone();

    let gauge_id = 0;

    // vote for one of the options in gauge
    suite
        .place_vote(
            &gauge_contract,
            voter1.to_owned(),
            gauge_id,
            Some(voter1.to_owned()), // option to vote for
        )
        .unwrap();
    suite
        .place_vote(
            &gauge_contract,
            voter2.to_owned(),
            gauge_id,
            Some(voter1.to_owned()),
        )
        .unwrap();

    let selected_set = suite.query_selected_set(&gauge_contract, gauge_id).unwrap();
    // voter1 was option voted for with two 100 voting powers combined
    assert_eq!(selected_set, vec![("voter1".to_owned(), Uint128::new(200))]);

    // before advancing specified epoch tally won't get sampled
    suite.advance_time(EPOCH);

    suite
        .execute_options(&gauge_contract, voter1, gauge_id)
        .unwrap();

    assert_eq!(
        suite.query_balance(voter1, reward_to_distribute.1).unwrap(),
        1000u128
    );
}

/// Small helper method to setup the gauge contract.
/// Make sure that `voter` has voting power.
fn init_gauge(suite: &mut Suite, voters: &[&str]) -> Addr {
    suite.next_block();
    suite
        .propose_update_proposal_module(voters[0], None)
        .unwrap();
    suite.next_block();
    let proposal = suite.list_proposals().unwrap()[0];
    for voter in voters {
        suite
            .place_vote_single(*voter, proposal, Vote::Yes)
            .unwrap();
    }
    suite.next_block();
    suite.execute_single_proposal(voters[0], proposal).unwrap();
    let proposal_modules = suite.query_proposal_modules().unwrap();

    // Second proposal module is cw proposal single, first one is newly added gauge
    assert_eq!(proposal_modules.len(), 2);
    proposal_modules[1].clone()
}

#[test]
fn query_last_execution() {
    let voter1 = "voter1";
    let voter2 = "voter2";

    let reward_to_distribute = (2000, "ujuno");

    let mut suite = SuiteBuilder::new()
        .with_voting_members(&[(voter1, 100), (voter2, 100)])
        .with_core_balance(reward_to_distribute)
        .build();

    let gauge_contract = init_gauge(&mut suite, &[voter1, voter2]);

    suite
        .instantiate_adapter_and_create_gauge(
            gauge_contract.clone(),
            &[voter1, voter2, gauge_contract.as_str()],
            (1000, "ujuno"),
            None,
            None,
            None,
        )
        .unwrap();
    let gauge_id = 0;

    assert_eq!(
        suite
            .query_last_executed_set(&gauge_contract, gauge_id)
            .unwrap(),
        None,
        "not executed yet"
    );

    // vote
    suite
        .place_vote(&gauge_contract, voter1, gauge_id, Some(voter1.to_owned()))
        .unwrap();
    suite
        .place_votes(
            &gauge_contract,
            voter2,
            gauge_id,
            vec![
                (gauge_contract.to_string(), Decimal::percent(40)),
                (voter2.to_owned(), Decimal::percent(60)),
            ],
        )
        .unwrap();
    // wait until epoch passes
    suite.advance_time(EPOCH);
    // execute
    suite
        .execute_options(&gauge_contract, voter1, gauge_id)
        .unwrap();

    // should return the executed set now
    let expected_votes = Some(vec![
        (voter1.to_owned(), 100u128.into()),
        (voter2.to_string(), 60u128.into()),
        (gauge_contract.to_string(), 40u128.into()),
    ]);
    assert_eq!(
        suite
            .query_last_executed_set(&gauge_contract, gauge_id)
            .unwrap(),
        expected_votes
    );

    // change votes
    suite
        .place_vote(&gauge_contract, voter1, gauge_id, Some(voter2.to_owned()))
        .unwrap();
    suite
        .place_vote(&gauge_contract, voter2, gauge_id, None)
        .unwrap();

    // wait until epoch passes
    suite.advance_time(EPOCH);

    // should not change last execution yet
    assert_eq!(
        suite
            .query_last_executed_set(&gauge_contract, gauge_id)
            .unwrap(),
        expected_votes
    );

    // execute
    suite
        .execute_options(&gauge_contract, voter1, gauge_id)
        .unwrap();

    // now it should be changed
    assert_eq!(
        suite
            .query_last_executed_set(&gauge_contract, gauge_id)
            .unwrap(),
        Some(vec![(voter2.to_owned(), 100u128.into())])
    );
}

#[test]
fn execute_gauge_twice_same_epoch() {
    let voter1 = "voter1";
    let voter2 = "voter2";
    let reward_to_distribute = (2000, "ujuno");
    let mut suite = SuiteBuilder::new()
        .with_voting_members(&[(voter1, 100), (voter2, 100)])
        .with_core_balance(reward_to_distribute)
        .build();

    suite.next_block();
    let gauge_config = suite
        .instantiate_adapter_and_return_config(&[voter1, voter2], (1000, "ujuno"), None, None, None) // reward per
        // epoch
        .unwrap();
    suite
        .propose_update_proposal_module(voter1.to_string(), vec![gauge_config])
        .unwrap();

    suite.next_block();
    let proposal = suite.list_proposals().unwrap()[0];
    suite
        .place_vote_single(voter1, proposal, Vote::Yes)
        .unwrap();
    suite
        .place_vote_single(voter2, proposal, Vote::Yes)
        .unwrap();

    suite.next_block();
    suite
        .execute_single_proposal(voter1.to_string(), proposal)
        .unwrap();
    let proposal_modules = suite.query_proposal_modules().unwrap();
    let gauge_contract = proposal_modules[1].clone();

    let gauge_id = 0;

    // vote for one of the options in gauge
    suite
        .place_vote(
            &gauge_contract,
            voter1.to_owned(),
            gauge_id,
            Some(voter1.to_owned()), // option to vote for
        )
        .unwrap();
    suite
        .place_vote(
            &gauge_contract,
            voter2.to_owned(),
            gauge_id,
            Some(voter1.to_owned()),
        )
        .unwrap();

    let selected_set = suite.query_selected_set(&gauge_contract, gauge_id).unwrap();
    // voter1 was option voted for with two 100 voting powers combined
    assert_eq!(selected_set, vec![("voter1".to_owned(), Uint128::new(200))]);

    // before advancing specified epoch tally won't get sampled
    suite.advance_time(EPOCH);

    suite
        .execute_options(&gauge_contract, voter1, gauge_id)
        .unwrap();

    assert_eq!(
        suite.query_balance(voter1, reward_to_distribute.1).unwrap(),
        1000u128
    );

    // execution twice same time won't work
    let err = suite
        .execute_options(&gauge_contract, voter1, gauge_id)
        .unwrap_err();
    let next_epoch = suite.current_time() + EPOCH;
    assert_eq!(
        ContractError::EpochNotReached {
            gauge_id,
            current_epoch: suite.current_time(),
            next_epoch
        },
        err.downcast().unwrap()
    );

    // just before next epoch fails as well
    suite.advance_time(EPOCH - 1);
    let err = suite
        .execute_options(&gauge_contract, voter1, gauge_id)
        .unwrap_err();
    assert_eq!(
        ContractError::EpochNotReached {
            gauge_id,
            current_epoch: suite.current_time(),
            next_epoch
        },
        err.downcast().unwrap()
    );

    // another epoch is fine
    suite.advance_time(EPOCH);
    suite
        .execute_options(&gauge_contract, voter1, gauge_id)
        .unwrap();

    assert_eq!(
        suite.query_balance(voter1, reward_to_distribute.1).unwrap(),
        2000u128
    );
}

#[test]
fn execute_stopped_gauge() {
    let voter1 = "voter1";
    let voter2 = "voter2";
    let reward_to_distribute = (1000, "ujuno");
    let mut suite = SuiteBuilder::new()
        .with_voting_members(&[(voter1, 100), (voter2, 100)])
        .with_core_balance(reward_to_distribute)
        .build();

    suite.next_block();
    let gauge_config = suite
        .instantiate_adapter_and_return_config(
            &[voter1, voter2],
            reward_to_distribute,
            None,
            None,
            None,
        )
        .unwrap();
    suite
        .propose_update_proposal_module(voter1.to_string(), vec![gauge_config])
        .unwrap();

    suite.next_block();
    let proposal = suite.list_proposals().unwrap()[0];
    suite
        .place_vote_single(voter1, proposal, Vote::Yes)
        .unwrap();
    suite
        .place_vote_single(voter2, proposal, Vote::Yes)
        .unwrap();

    suite.next_block();
    suite
        .execute_single_proposal(voter1.to_string(), proposal)
        .unwrap();
    let proposal_modules = suite.query_proposal_modules().unwrap();
    let gauge_contract = proposal_modules[1].clone();

    let gauge_id = 0;

    // stop the gauge by not-owner
    let err = suite
        .stop_gauge(&gauge_contract, voter1, gauge_id)
        .unwrap_err();
    assert_eq!(
        ContractError::Ownership(cw_ownable::OwnershipError::NotOwner),
        err.downcast().unwrap()
    );

    // stop the gauge by owner
    suite
        .stop_gauge(&gauge_contract, suite.core.clone(), gauge_id)
        .unwrap();

    // vote for one of the options in gauge
    suite
        .place_vote(
            &gauge_contract,
            voter1.to_owned(),
            gauge_id,
            Some(voter1.to_owned()), // option to vote for
        )
        .unwrap();
    suite
        .place_vote(
            &gauge_contract,
            voter2.to_owned(),
            gauge_id,
            Some(voter1.to_owned()),
        )
        .unwrap();

    // Despite gauge being stopped, user
    let selected_set = suite.query_selected_set(&gauge_contract, gauge_id).unwrap();
    assert_eq!(selected_set, vec![("voter1".to_owned(), Uint128::new(200))]);

    // before advancing specified epoch tally won't get sampled
    suite.advance_time(EPOCH);

    let err = suite
        .execute_options(&gauge_contract, voter1, gauge_id)
        .unwrap_err();
    assert_eq!(
        ContractError::GaugeStopped(gauge_id),
        err.downcast().unwrap()
    );
}

#[test]
fn update_gauge() {
    let voter1 = "voter1";
    let voter2 = "voter2";
    let mut suite = SuiteBuilder::new()
        .with_voting_members(&[(voter1, 100), (voter2, 100)])
        .build();

    let gauge_contract = init_gauge(&mut suite, &[voter1, voter2]);

    let gauge_adapter = suite
        .instantiate_adapter_and_create_gauge(
            gauge_contract.clone(),
            &[voter1, voter2],
            (1000, "ujuno"),
            None,
            None,
            None,
        )
        .unwrap();

    let second_gauge_adapter = suite
        .instantiate_adapter_and_create_gauge(
            gauge_contract.clone(),
            &[voter1, voter2],
            (1000, "uusdc"),
            None,
            None,
            None,
        )
        .unwrap();

    let response = suite.query_gauges(gauge_contract.clone()).unwrap();
    assert_eq!(
        response,
        vec![
            GaugeResponse {
                id: 0,
                title: "gauge".to_owned(),
                adapter: gauge_adapter.to_string(),
                epoch_size: EPOCH,
                min_percent_selected: Some(Decimal::percent(5)),
                max_options_selected: 10,
                max_available_percentage: None,
                is_stopped: false,
                next_epoch: suite.current_time() + 7 * 86400,
                reset: None,
                total_epochs: None
            },
            GaugeResponse {
                id: 1,
                title: "gauge".to_owned(),
                adapter: second_gauge_adapter.to_string(),
                epoch_size: EPOCH,
                min_percent_selected: Some(Decimal::percent(5)),
                max_options_selected: 10,
                max_available_percentage: None,
                is_stopped: false,
                next_epoch: suite.current_time() + 7 * 86400,
                reset: None,
                total_epochs: None
            }
        ]
    );

    // update parameters on the first gauge
    let dao = suite.core.clone();
    let new_epoch = EPOCH * 2;
    let epoch_limit = 8u64;
    let new_min_percent = Some(Decimal::percent(10));
    let new_max_options = 15;
    let new_max_available_percentage = Some(Decimal::percent(5));
    suite
        .update_gauge(
            dao.as_str(),
            gauge_contract.clone(),
            0,
            new_epoch,
            epoch_limit,
            new_min_percent,
            new_max_options,
            new_max_available_percentage,
        )
        .unwrap();

    let response = suite.query_gauges(gauge_contract.clone()).unwrap();
    assert_eq!(
        response,
        vec![
            GaugeResponse {
                id: 0,
                title: "gauge".to_owned(),
                adapter: gauge_adapter.to_string(),
                epoch_size: new_epoch,
                min_percent_selected: new_min_percent,
                max_options_selected: new_max_options,
                max_available_percentage: new_max_available_percentage,
                is_stopped: false,
                next_epoch: suite.current_time() + 7 * 86400,
                reset: None,
                total_epochs: None
            },
            GaugeResponse {
                id: 1,
                title: "gauge".to_owned(),
                adapter: second_gauge_adapter.to_string(),
                epoch_size: EPOCH,
                min_percent_selected: Some(Decimal::percent(5)),
                max_options_selected: 10,
                max_available_percentage: None,
                is_stopped: false,
                next_epoch: suite.current_time() + 7 * 86400,
                reset: None,
                total_epochs: None
            }
        ]
    );

    // clean setting of min_percent_selected on second gauge
    suite
        .update_gauge(
            dao.as_str(),
            gauge_contract.clone(),
            1,
            None,
            epoch_limit,
            Some(Decimal::zero()),
            None,
            None,
        )
        .unwrap();

    let response = suite.query_gauges(gauge_contract.clone()).unwrap();
    assert_eq!(
        response,
        vec![
            GaugeResponse {
                id: 0,
                title: "gauge".to_owned(),
                adapter: gauge_adapter.to_string(),
                epoch_size: new_epoch,
                min_percent_selected: new_min_percent,
                max_options_selected: new_max_options,
                max_available_percentage: new_max_available_percentage,
                is_stopped: false,
                next_epoch: suite.current_time() + 7 * 86400,
                reset: None,
                total_epochs: None
            },
            GaugeResponse {
                id: 1,
                title: "gauge".to_owned(),
                adapter: second_gauge_adapter.to_string(),
                epoch_size: EPOCH,
                min_percent_selected: None,
                max_options_selected: 10,
                max_available_percentage: None,
                is_stopped: false,
                next_epoch: suite.current_time() + 7 * 86400,
                reset: None,
                total_epochs: None
            }
        ]
    );

    // Not owner cannot update gauges
    let err = suite
        .update_gauge(
            "notowner",
            gauge_contract.clone(),
            0,
            new_epoch,
            epoch_limit,
            new_min_percent,
            new_max_options,
            None,
        )
        .unwrap_err();
    assert_eq!(
        ContractError::Ownership(cw_ownable::OwnershipError::NotOwner),
        err.downcast().unwrap()
    );

    let err = suite
        .update_gauge(
            dao.as_str(),
            gauge_contract.clone(),
            0,
            50,
            epoch_limit,
            new_min_percent,
            new_max_options,
            None,
        )
        .unwrap_err();
    assert_eq!(ContractError::EpochSizeTooShort {}, err.downcast().unwrap());

    let err = suite
        .update_gauge(
            dao.as_str(),
            gauge_contract.clone(),
            0,
            new_epoch,
            epoch_limit,
            Some(Decimal::one()),
            new_max_options,
            None,
        )
        .unwrap_err();
    assert_eq!(
        ContractError::MinPercentSelectedTooBig {},
        err.downcast().unwrap()
    );

    let err = suite
        .update_gauge(
            dao.as_str(),
            gauge_contract.clone(),
            0,
            new_epoch,
            epoch_limit,
            new_min_percent,
            0,
            None,
        )
        .unwrap_err();
    assert_eq!(
        ContractError::MaxOptionsSelectedTooSmall {},
        err.downcast().unwrap()
    );

    let err = suite
        .update_gauge(
            dao.as_str(),
            gauge_contract,
            1,
            None,
            epoch_limit,
            Some(Decimal::zero()),
            None,
            Some(Decimal::percent(101)),
        )
        .unwrap_err();
    assert_eq!(
        ContractError::MaxAvailablePercentTooBig {},
        err.downcast().unwrap()
    );
}
