mod setup;

use crate::setup::*;
use ft_lockup::lockup::Lockup;
use ft_lockup::schedule::{Checkpoint, Schedule};
use ft_lockup::termination::{HashOrSchedule, TerminationConfig};
use near_sdk::json_types::WrappedBalance;

const ONE_DAY_SEC: TimestampSec = 24 * 60 * 60;
const ONE_YEAR_SEC: TimestampSec = 365 * ONE_DAY_SEC;

const GENESIS_TIMESTAMP_SEC: TimestampSec = 1_600_000_000;

#[test]
fn test_init_env() {
    let e = Env::init(None);
    let _users = Users::init(&e);
}

#[test]
fn test_lockup_claim_logic() {
    let e = Env::init(None);
    let users = Users::init(&e);
    let amount = d(10000, TOKEN_DECIMALS);
    e.set_time_sec(GENESIS_TIMESTAMP_SEC);
    let lockups = e.get_account_lockups(&users.alice);
    assert!(lockups.is_empty());
    let lockup = Lockup {
        account_id: users.alice.valid_account_id(),
        schedule: Schedule(vec![
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC - 1,
                balance: 0,
            },
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC,
                balance: amount,
            },
        ]),
        claimed_balance: 0,
        termination_config: None,
    };
    let balance: WrappedBalance = e.add_lockup(&e.owner, amount, &lockup).unwrap_json();
    assert_eq!(balance.0, amount);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups.len(), 1);
    assert_eq!(lockups[0].1.total_balance, amount);
    assert_eq!(lockups[0].1.claimed_balance, 0);
    assert_eq!(lockups[0].1.unclaimed_balance, 0);

    // Claim attempt before unlock.
    let res: WrappedBalance = e.claim(&users.alice).unwrap_json();
    assert_eq!(res.0, 0);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.claimed_balance, 0);

    // Set time to the first checkpoint.
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC - 1);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.claimed_balance, 0);
    assert_eq!(lockups[0].1.unclaimed_balance, 0);

    // Set time to the second checkpoint.
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.claimed_balance, 0);
    assert_eq!(lockups[0].1.unclaimed_balance, amount);

    // Attempt to claim. No storage deposit for Alice.
    let res: WrappedBalance = e.claim(&users.alice).unwrap_json();
    assert_eq!(res.0, 0);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.claimed_balance, 0);
    assert_eq!(lockups[0].1.unclaimed_balance, amount);

    ft_storage_deposit(&users.alice, TOKEN_ID, &users.alice.account_id);

    let balance = e.ft_balance_of(&users.alice);
    assert_eq!(balance, 0);

    // Claim tokens.
    let res: WrappedBalance = e.claim(&users.alice).unwrap_json();
    assert_eq!(res.0, amount);
    // User's lockups should be empty, since fully claimed.
    let lockups = e.get_account_lockups(&users.alice);
    assert!(lockups.is_empty());

    // Manually checking the lockup by index
    let lockup = e.get_lockup(0);
    assert_eq!(lockup.claimed_balance, amount);
    assert_eq!(lockup.unclaimed_balance, 0);

    let balance = e.ft_balance_of(&users.alice);
    assert_eq!(balance, amount);
}

#[test]
fn test_lockup_linear() {
    let e = Env::init(None);
    let users = Users::init(&e);
    let amount = d(60000, TOKEN_DECIMALS);
    e.set_time_sec(GENESIS_TIMESTAMP_SEC);
    let lockups = e.get_account_lockups(&users.alice);
    assert!(lockups.is_empty());
    let lockup = Lockup {
        account_id: users.alice.valid_account_id(),
        schedule: Schedule(vec![
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC,
                balance: 0,
            },
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC,
                balance: amount,
            },
        ]),
        claimed_balance: 0,
        termination_config: None,
    };
    let balance: WrappedBalance = e.add_lockup(&e.owner, amount, &lockup).unwrap_json();
    assert_eq!(balance.0, amount);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups.len(), 1);
    assert_eq!(lockups[0].1.total_balance, amount);
    assert_eq!(lockups[0].1.claimed_balance, 0);
    assert_eq!(lockups[0].1.unclaimed_balance, 0);

    // 1/3 unlock
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC / 3);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.total_balance, amount);
    assert_eq!(lockups[0].1.claimed_balance, 0);
    assert_eq!(lockups[0].1.unclaimed_balance, amount / 3);

    // Claim tokens
    ft_storage_deposit(&users.alice, TOKEN_ID, &users.alice.account_id);
    let res: WrappedBalance = e.claim(&users.alice).unwrap_json();
    assert_eq!(res.0, amount / 3);
    let balance = e.ft_balance_of(&users.alice);
    assert_eq!(balance, amount / 3);

    // Check lockup after claim
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.total_balance, amount);
    assert_eq!(lockups[0].1.claimed_balance, amount / 3);
    assert_eq!(lockups[0].1.unclaimed_balance, 0);

    // 1/2 unlock
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC / 2);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.total_balance, amount);
    assert_eq!(lockups[0].1.claimed_balance, amount / 3);
    assert_eq!(lockups[0].1.unclaimed_balance, amount / 6);

    // Remove storage from token to verify claim refund.
    // Note, this burns `amount / 3` tokens.
    storage_force_unregister(&users.alice, TOKEN_ID);
    let balance = e.ft_balance_of(&users.alice);
    assert_eq!(balance, 0);

    // Trying to claim, should fail and refund the amount back to the lockup
    let res: WrappedBalance = e.claim(&users.alice).unwrap_json();
    assert_eq!(res.0, 0);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.total_balance, amount);
    assert_eq!(lockups[0].1.claimed_balance, amount / 3);
    assert_eq!(lockups[0].1.unclaimed_balance, amount / 6);

    // Claim again but with storage deposit
    ft_storage_deposit(&users.alice, TOKEN_ID, &users.alice.account_id);
    let res: WrappedBalance = e.claim(&users.alice).unwrap_json();
    assert_eq!(res.0, amount / 6);
    let balance = e.ft_balance_of(&users.alice);
    assert_eq!(balance, amount / 6);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.total_balance, amount);
    assert_eq!(lockups[0].1.claimed_balance, amount / 2);
    assert_eq!(lockups[0].1.unclaimed_balance, 0);

    // 2/3 unlock
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 2 / 3);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.claimed_balance, amount / 2);
    assert_eq!(lockups[0].1.unclaimed_balance, amount / 6);

    // Claim tokens
    let res: WrappedBalance = e.claim(&users.alice).unwrap_json();
    assert_eq!(res.0, amount / 6);
    let balance = e.ft_balance_of(&users.alice);
    assert_eq!(balance, amount / 3);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.claimed_balance, amount * 2 / 3);
    assert_eq!(lockups[0].1.unclaimed_balance, 0);

    // Claim again with no unclaimed_balance
    let res: WrappedBalance = e.claim(&users.alice).unwrap_json();
    assert_eq!(res.0, 0);
    let balance = e.ft_balance_of(&users.alice);
    assert_eq!(balance, amount / 3);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.claimed_balance, amount * 2 / 3);
    assert_eq!(lockups[0].1.unclaimed_balance, 0);

    // full unlock
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.claimed_balance, amount * 2 / 3);
    assert_eq!(lockups[0].1.unclaimed_balance, amount / 3);

    // Final claim
    let res: WrappedBalance = e.claim(&users.alice).unwrap_json();
    assert_eq!(res.0, amount / 3);
    let balance = e.ft_balance_of(&users.alice);
    assert_eq!(balance, amount * 2 / 3);

    // User's lockups should be empty, since fully claimed.
    let lockups = e.get_account_lockups(&users.alice);
    assert!(lockups.is_empty());

    // Manually checking the lockup by index
    let lockup = e.get_lockup(0);
    assert_eq!(lockup.claimed_balance, amount);
    assert_eq!(lockup.unclaimed_balance, 0);
}

#[test]
fn test_lockup_cliff_amazon() {
    let e = Env::init(None);
    let users = Users::init(&e);
    let amount = d(60000, TOKEN_DECIMALS);
    e.set_time_sec(GENESIS_TIMESTAMP_SEC);
    let lockups = e.get_account_lockups(&users.alice);
    assert!(lockups.is_empty());
    let lockup = Lockup {
        account_id: users.alice.valid_account_id(),
        schedule: Schedule(vec![
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC - 1,
                balance: 0,
            },
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC,
                balance: amount / 10,
            },
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 2,
                balance: 3 * amount / 10,
            },
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 3,
                balance: 6 * amount / 10,
            },
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 4,
                balance: amount,
            },
        ]),
        claimed_balance: 0,
        termination_config: None,
    };
    let balance: WrappedBalance = e.add_lockup(&e.owner, amount, &lockup).unwrap_json();
    assert_eq!(balance.0, amount);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups.len(), 1);
    assert_eq!(lockups[0].1.total_balance, amount);
    assert_eq!(lockups[0].1.claimed_balance, 0);
    assert_eq!(lockups[0].1.unclaimed_balance, 0);

    // 1/12 time. pre-cliff unlock
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC / 3);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.total_balance, amount);
    assert_eq!(lockups[0].1.claimed_balance, 0);
    assert_eq!(lockups[0].1.unclaimed_balance, 0);

    // 1/4 time. cliff unlock
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.total_balance, amount);
    assert_eq!(lockups[0].1.claimed_balance, 0);
    assert_eq!(lockups[0].1.unclaimed_balance, amount / 10);

    // 3/8 time. cliff unlock + 1/2 of 2nd year.
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC + ONE_YEAR_SEC / 2);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.unclaimed_balance, 2 * amount / 10);

    // 1/2 time.
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 2);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.unclaimed_balance, 3 * amount / 10);

    // 1/2 + 1/12 time.
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 2 + ONE_YEAR_SEC / 3);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.unclaimed_balance, 4 * amount / 10);

    // 1/2 + 2/12 time.
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 2 + ONE_YEAR_SEC * 2 / 3);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.unclaimed_balance, 5 * amount / 10);

    // 3/4 time.
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 3);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.unclaimed_balance, 6 * amount / 10);

    // 7/8 time.
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 3 + ONE_YEAR_SEC / 2);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.unclaimed_balance, 8 * amount / 10);

    // full unlock.
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 4);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.unclaimed_balance, amount);

    // after unlock.
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 5);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.unclaimed_balance, amount);

    // attempt to claim without storage.
    let res: WrappedBalance = e.claim(&users.alice).unwrap_json();
    assert_eq!(res.0, 0);
    let balance = e.ft_balance_of(&users.alice);
    assert_eq!(balance, 0);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.unclaimed_balance, amount);

    // Claim tokens
    ft_storage_deposit(&users.alice, TOKEN_ID, &users.alice.account_id);
    let res: WrappedBalance = e.claim(&users.alice).unwrap_json();
    assert_eq!(res.0, amount);
    let balance = e.ft_balance_of(&users.alice);
    assert_eq!(balance, amount);

    // Check lockup after claim
    let lockups = e.get_account_lockups(&users.alice);
    assert!(lockups.is_empty());
    let lockup = e.get_lockup(0);
    assert_eq!(lockup.claimed_balance, amount);
    assert_eq!(lockup.unclaimed_balance, 0);
}

#[test]
fn test_lockup_terminate_no_vesting_schedule() {
    let e = Env::init(None);
    let users = Users::init(&e);
    let amount = d(60000, TOKEN_DECIMALS);
    e.set_time_sec(GENESIS_TIMESTAMP_SEC);
    let lockups = e.get_account_lockups(&users.alice);
    assert!(lockups.is_empty());
    let lockup = Lockup {
        account_id: users.alice.valid_account_id(),
        schedule: Schedule(vec![
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC,
                balance: 0,
            },
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC,
                balance: amount,
            },
        ]),
        claimed_balance: 0,
        termination_config: Some(TerminationConfig {
            terminator_id: users.eve.valid_account_id(),
            vesting_schedule: None,
        }),
    };

    let balance: WrappedBalance = e.add_lockup(&e.owner, amount, &lockup).unwrap_json();
    assert_eq!(balance.0, amount);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups.len(), 1);
    assert_eq!(lockups[0].1.total_balance, amount);
    assert_eq!(lockups[0].1.claimed_balance, 0);
    assert_eq!(lockups[0].1.unclaimed_balance, 0);

    // 1/3 unlock
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC / 3);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.total_balance, amount);
    assert_eq!(lockups[0].1.claimed_balance, 0);
    assert_eq!(lockups[0].1.unclaimed_balance, amount / 3);

    // Claim tokens
    ft_storage_deposit(&users.alice, TOKEN_ID, &users.alice.account_id);
    let res: WrappedBalance = e.claim(&users.alice).unwrap_json();
    assert_eq!(res.0, amount / 3);
    let balance = e.ft_balance_of(&users.alice);
    assert_eq!(balance, amount / 3);

    // Check lockup after claim
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.total_balance, amount);
    assert_eq!(lockups[0].1.claimed_balance, amount / 3);
    assert_eq!(lockups[0].1.unclaimed_balance, 0);

    // 1/2 unlock
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC / 2);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.total_balance, amount);
    assert_eq!(lockups[0].1.claimed_balance, amount / 3);
    assert_eq!(lockups[0].1.unclaimed_balance, amount / 6);

    let lockup_index = lockups[0].0;

    // TERMINATE
    ft_storage_deposit(&users.eve, TOKEN_ID, &users.eve.account_id);
    let res: WrappedBalance = e.terminate(&users.eve, lockup_index).unwrap_json();
    assert_eq!(res.0, amount / 2);

    let terminator_balance = e.ft_balance_of(&users.eve);
    assert_eq!(terminator_balance, amount / 2);

    // full unlock 2 / 3 period after termination before initial timestamp
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 2 / 3);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.claimed_balance, amount / 3);
    assert_eq!(lockups[0].1.unclaimed_balance, amount / 6);

    // Final claim
    let res: WrappedBalance = e.claim(&users.alice).unwrap_json();
    assert_eq!(res.0, amount / 6);
    let balance = e.ft_balance_of(&users.alice);
    assert_eq!(balance, amount / 2);

    // User's lockups should be empty, since fully claimed.
    let lockups = e.get_account_lockups(&users.alice);
    assert!(lockups.is_empty());

    // Manually checking the lockup by index
    let lockup = e.get_lockup(0);
    assert_eq!(lockup.claimed_balance, amount / 2);
    assert_eq!(lockup.unclaimed_balance, 0);
}

#[test]
fn test_lockup_terminate_no_termination_config() {
    let e = Env::init(None);
    let users = Users::init(&e);
    let amount = d(60000, TOKEN_DECIMALS);
    e.set_time_sec(GENESIS_TIMESTAMP_SEC);
    let lockups = e.get_account_lockups(&users.alice);
    assert!(lockups.is_empty());
    let lockup = Lockup {
        account_id: users.alice.valid_account_id(),
        schedule: Schedule(vec![
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC,
                balance: 0,
            },
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC,
                balance: amount,
            },
        ]),
        claimed_balance: 0,
        termination_config: None,
    };

    let balance: WrappedBalance = e.add_lockup(&e.owner, amount, &lockup).unwrap_json();
    assert_eq!(balance.0, amount);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups.len(), 1);
    let lockup_index = lockups[0].0;
    assert_eq!(lockups[0].1.total_balance, amount);
    assert_eq!(lockups[0].1.claimed_balance, 0);
    assert_eq!(lockups[0].1.unclaimed_balance, 0);

    // TERMINATE
    ft_storage_deposit(&users.eve, TOKEN_ID, &users.eve.account_id);
    let res = e.terminate(&users.eve, lockup_index);
    assert!(!res.is_ok());
    assert!(format!("{:?}", res.status()).contains("No termination config"));
}

#[test]
fn test_lockup_terminate_wrong_terminator() {
    let e = Env::init(None);
    let users = Users::init(&e);
    let amount = d(60000, TOKEN_DECIMALS);
    e.set_time_sec(GENESIS_TIMESTAMP_SEC);
    let lockups = e.get_account_lockups(&users.alice);
    assert!(lockups.is_empty());
    let lockup = Lockup {
        account_id: users.alice.valid_account_id(),
        schedule: Schedule(vec![
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC,
                balance: 0,
            },
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC,
                balance: amount,
            },
        ]),
        claimed_balance: 0,
        termination_config: Some(TerminationConfig {
            terminator_id: users.eve.valid_account_id(),
            vesting_schedule: None,
        }),
    };

    let balance: WrappedBalance = e.add_lockup(&e.owner, amount, &lockup).unwrap_json();
    assert_eq!(balance.0, amount);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups.len(), 1);
    let lockup_index = lockups[0].0;
    assert_eq!(lockups[0].1.total_balance, amount);
    assert_eq!(lockups[0].1.claimed_balance, 0);
    assert_eq!(lockups[0].1.unclaimed_balance, 0);

    // TERMINATE
    ft_storage_deposit(&users.dude, TOKEN_ID, &users.dude.account_id);
    let res = e.terminate(&users.dude, lockup_index);
    assert!(!res.is_ok());
    assert!(format!("{:?}", res.status()).contains("Unauthorized"));
}

#[test]
fn test_lockup_terminate_no_storage() {
    let e = Env::init(None);
    let users = Users::init(&e);
    let amount = d(60000, TOKEN_DECIMALS);
    e.set_time_sec(GENESIS_TIMESTAMP_SEC);
    let lockups = e.get_account_lockups(&users.alice);
    assert!(lockups.is_empty());
    let lockup = Lockup {
        account_id: users.alice.valid_account_id(),
        schedule: Schedule(vec![
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC,
                balance: 0,
            },
            Checkpoint {
                timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC,
                balance: amount,
            },
        ]),
        claimed_balance: 0,
        termination_config: Some(TerminationConfig {
            terminator_id: users.eve.valid_account_id(),
            vesting_schedule: None,
        }),
    };

    let balance: WrappedBalance = e.add_lockup(&e.owner, amount, &lockup).unwrap_json();
    assert_eq!(balance.0, amount);
    let lockups = e.get_account_lockups(&users.alice);
    let lockup_index = lockups[0].0;

    // 1/3 unlock, terminate
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC / 3);
    // Claim tokens
    // TERMINATE, without deposit must create unlocked lockup for terminator
    let res: WrappedBalance = e.terminate(&users.eve, lockup_index).unwrap_json();
    assert_eq!(res.0, 0);

    ft_storage_deposit(&users.eve, TOKEN_ID, &users.eve.account_id);
    let terminator_balance = e.ft_balance_of(&users.eve);
    assert_eq!(terminator_balance, 0);

    {
        let lockups = e.get_account_lockups(&users.eve);
        assert_eq!(lockups.len(), 1);
        assert_eq!(lockups[0].1.claimed_balance, 0);
        assert_eq!(lockups[0].1.unclaimed_balance, amount * 2 / 3);
        let terminator_lockup_index = lockups[0].0;

        // Claim from lockup refund
        let res: WrappedBalance = e.claim(&users.eve).unwrap_json();
        assert_eq!(res.0, amount * 2 / 3);
        let balance = e.ft_balance_of(&users.eve);
        assert_eq!(balance, amount * 2 / 3);

        // Terminator's lockups should be empty, since fully claimed.
        let lockups = e.get_account_lockups(&users.eve);
        assert!(lockups.is_empty());

        // Manually checking the terminator's lockup by index
        let lockup = e.get_lockup(terminator_lockup_index);
        assert_eq!(lockup.claimed_balance, amount * 2 / 3);
        assert_eq!(lockup.unclaimed_balance, 0);
    }

    {
        let lockups = e.get_account_lockups(&users.alice);
        assert_eq!(lockups.len(), 1);
        assert_eq!(lockups[0].1.claimed_balance, 0);
        assert_eq!(lockups[0].1.unclaimed_balance, amount / 3);

        // Claim by user
        ft_storage_deposit(&users.alice, TOKEN_ID, &users.alice.account_id);
        let balance = e.ft_balance_of(&users.alice);
        assert_eq!(balance, 0);

        let res: WrappedBalance = e.claim(&users.alice).unwrap_json();
        assert_eq!(res.0, amount / 3);
        let balance = e.ft_balance_of(&users.alice);
        assert_eq!(balance, amount / 3);

        // User's lockups should be empty, since fully claimed.
        let lockups = e.get_account_lockups(&users.alice);
        assert!(lockups.is_empty());

        // Manually checking the terminator's lockup by index
        let lockup = e.get_lockup(lockup_index);
        assert_eq!(lockup.claimed_balance, amount / 3);
        assert_eq!(lockup.unclaimed_balance, 0);
    }
}

fn lockup_vesting_schedule(amount: u128) -> (Schedule, Schedule) {
    let lockup_schedule = Schedule(vec![
        Checkpoint {
            timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 2,
            balance: 0,
        },
        Checkpoint {
            timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 4,
            balance: amount * 3 / 4,
        },
        Checkpoint {
            timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 4 + 1,
            balance: amount,
        },
    ]);
    let vesting_schedule = Schedule(vec![
        Checkpoint {
            timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC - 1,
            balance: 0,
        },
        Checkpoint {
            timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC,
            balance: amount / 4,
        },
        Checkpoint {
            timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 4,
            balance: amount,
        },
    ]);
    (lockup_schedule, vesting_schedule)
}

#[test]
fn test_lockup_terminate_custom_vesting_hash() {
    let e = Env::init(None);
    let users = Users::init(&e);
    let amount = d(60000, TOKEN_DECIMALS);
    e.set_time_sec(GENESIS_TIMESTAMP_SEC);
    let lockups = e.get_account_lockups(&users.alice);
    assert!(lockups.is_empty());

    let (lockup_schedule, vesting_schedule) = lockup_vesting_schedule(amount);
    let vesting_hash = e.hash_schedule(&vesting_schedule);
    let lockup = Lockup {
        account_id: users.alice.valid_account_id(),
        schedule: lockup_schedule,
        claimed_balance: 0,
        termination_config: Some(TerminationConfig {
            terminator_id: users.eve.valid_account_id(),
            vesting_schedule: Some(HashOrSchedule::Hash(vesting_hash)),
        }),
    };

    let balance: WrappedBalance = e.add_lockup(&e.owner, amount, &lockup).unwrap_json();
    assert_eq!(balance.0, amount);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups.len(), 1);
    let lockup_index = lockups[0].0;

    // 1Y, 1 / 4 vested, 0 unlocked
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.total_balance, amount);
    assert_eq!(lockups[0].1.claimed_balance, 0);
    assert_eq!(lockups[0].1.unclaimed_balance, 0);

    // TERMINATE
    ft_storage_deposit(&users.eve, TOKEN_ID, &users.eve.account_id);
    let res: WrappedBalance = e
        .terminate_with_schedule(&users.eve, lockup_index, vesting_schedule)
        .unwrap_json();
    assert_eq!(res.0, amount * 3 / 4);
    let terminator_balance = e.ft_balance_of(&users.eve);
    assert_eq!(terminator_balance, amount * 3 / 4);

    // Checking lockup
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.total_balance, amount / 4);
    assert_eq!(lockups[0].1.claimed_balance, 0);
    assert_eq!(lockups[0].1.unclaimed_balance, 0);

    // Rewind to 2Y + Y * 2 / 3, 1/4 of original unlock, full vested unlock
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 2 + ONE_YEAR_SEC * 2 / 3);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.total_balance, amount / 4);
    assert_eq!(lockups[0].1.claimed_balance, 0);
    assert_eq!(lockups[0].1.unclaimed_balance, amount / 4);

    // claiming
    ft_storage_deposit(&users.alice, TOKEN_ID, &users.alice.account_id);
    let res: WrappedBalance = e.claim(&users.alice).unwrap_json();
    assert_eq!(res.0, amount / 4);

    // Checking lockups
    let lockups = e.get_account_lockups(&users.alice);
    assert!(lockups.is_empty());

    // User lockups are empty
    let lockup = e.get_lockup(lockup_index);
    assert_eq!(lockup.total_balance, amount / 4);
    assert_eq!(lockup.claimed_balance, amount / 4);
    assert_eq!(lockup.unclaimed_balance, 0);
}

#[test]
fn test_lockup_terminate_custom_vesting_invalid_hash() {
    let e = Env::init(None);
    let users = Users::init(&e);
    let amount = d(60000, TOKEN_DECIMALS);
    e.set_time_sec(GENESIS_TIMESTAMP_SEC);
    let lockups = e.get_account_lockups(&users.alice);
    assert!(lockups.is_empty());

    let (lockup_schedule, vesting_schedule) = lockup_vesting_schedule(amount);
    let vesting_hash = e.hash_schedule(&vesting_schedule);
    let lockup = Lockup {
        account_id: users.alice.valid_account_id(),
        schedule: lockup_schedule,
        claimed_balance: 0,
        termination_config: Some(TerminationConfig {
            terminator_id: users.eve.valid_account_id(),
            vesting_schedule: Some(HashOrSchedule::Hash(vesting_hash)),
        }),
    };

    let balance: WrappedBalance = e.add_lockup(&e.owner, amount, &lockup).unwrap_json();
    assert_eq!(balance.0, amount);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups.len(), 1);
    let lockup_index = lockups[0].0;

    // 1Y, 1 / 4 vested, 0 unlocked
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.total_balance, amount);
    assert_eq!(lockups[0].1.claimed_balance, 0);
    assert_eq!(lockups[0].1.unclaimed_balance, 0);

    // TERMINATE
    let fake_schedule = Schedule(vec![
        Checkpoint {
            timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 2,
            balance: 0,
        },
        Checkpoint {
            timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 4,
            balance: amount,
        },
    ]);
    ft_storage_deposit(&users.eve, TOKEN_ID, &users.eve.account_id);
    let res = e.terminate_with_schedule(&users.eve, lockup_index, fake_schedule);
    assert!(!res.is_ok());
    assert!(format!("{:?}", res.status()).contains("The revealed schedule hash doesn't match"));
}

#[test]
fn test_lockup_terminate_custom_vesting_incompatible_vesting_schedule_by_hash() {
    let e = Env::init(None);
    let users = Users::init(&e);
    let amount = d(60000, TOKEN_DECIMALS);
    e.set_time_sec(GENESIS_TIMESTAMP_SEC);
    let lockups = e.get_account_lockups(&users.alice);
    assert!(lockups.is_empty());

    let (lockup_schedule, _vesting_schedule) = lockup_vesting_schedule(amount);
    let incompatible_vesting_schedule = Schedule(vec![
        Checkpoint {
            timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 4,
            balance: 0,
        },
        Checkpoint {
            timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 4 + 1,
            balance: amount,
        },
    ]);
    let incompatible_vesting_hash = e.hash_schedule(&incompatible_vesting_schedule);
    let lockup = Lockup {
        account_id: users.alice.valid_account_id(),
        schedule: lockup_schedule,
        claimed_balance: 0,
        termination_config: Some(TerminationConfig {
            terminator_id: users.eve.valid_account_id(),
            vesting_schedule: Some(HashOrSchedule::Hash(incompatible_vesting_hash)),
        }),
    };

    let balance: WrappedBalance = e.add_lockup(&e.owner, amount, &lockup).unwrap_json();
    assert_eq!(balance.0, amount);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups.len(), 1);
    let lockup_index = lockups[0].0;

    // 1Y, 1 / 4 vested, 0 unlocked
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.total_balance, amount);
    assert_eq!(lockups[0].1.claimed_balance, 0);
    assert_eq!(lockups[0].1.unclaimed_balance, 0);

    // TERMINATE
    ft_storage_deposit(&users.eve, TOKEN_ID, &users.eve.account_id);
    let res = e.terminate_with_schedule(&users.eve, lockup_index, incompatible_vesting_schedule);
    assert!(!res.is_ok());
    assert!(format!("{:?}", res.status()).contains("The lockup schedule is ahead of"));
}

#[test]
fn test_validate_schedule() {
    let e = Env::init(None);
    let users = Users::init(&e);
    let amount = d(60000, TOKEN_DECIMALS);
    e.set_time_sec(GENESIS_TIMESTAMP_SEC);
    let lockups = e.get_account_lockups(&users.alice);
    assert!(lockups.is_empty());

    let (lockup_schedule, vesting_schedule) = lockup_vesting_schedule(amount);

    let res = e.validate_schedule(&lockup_schedule, amount.into(), Some(&vesting_schedule));
    assert!(res.is_ok());

    let incompatible_vesting_schedule = Schedule(vec![
        Checkpoint {
            timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 4,
            balance: 0,
        },
        Checkpoint {
            timestamp: GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 4 + 1,
            balance: amount,
        },
    ]);
    let res = e.validate_schedule(
        &lockup_schedule,
        amount.into(),
        Some(&incompatible_vesting_schedule),
    );
    assert!(!res.is_ok());
    assert!(format!("{:?}", res.unwrap_err()).contains("The lockup schedule is ahead of"));
}

#[test]
fn test_lockup_terminate_custom_vesting_terminate_before_cliff() {
    let e = Env::init(None);
    let users = Users::init(&e);
    let amount = d(60000, TOKEN_DECIMALS);
    e.set_time_sec(GENESIS_TIMESTAMP_SEC);
    let lockups = e.get_account_lockups(&users.alice);
    assert!(lockups.is_empty());

    let (lockup_schedule, vesting_schedule) = lockup_vesting_schedule(amount);
    let lockup = Lockup {
        account_id: users.alice.valid_account_id(),
        schedule: lockup_schedule,
        claimed_balance: 0,
        termination_config: Some(TerminationConfig {
            terminator_id: users.eve.valid_account_id(),
            vesting_schedule: Some(HashOrSchedule::Schedule(vesting_schedule)),
        }),
    };

    let balance: WrappedBalance = e.add_lockup(&e.owner, amount, &lockup).unwrap_json();
    assert_eq!(balance.0, amount);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups.len(), 1);
    let lockup_index = lockups[0].0;

    // 1Y - 1 before cliff termination
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC - 1);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.total_balance, amount);
    assert_eq!(lockups[0].1.claimed_balance, 0);
    assert_eq!(lockups[0].1.unclaimed_balance, 0);

    // TERMINATE
    ft_storage_deposit(&users.eve, TOKEN_ID, &users.eve.account_id);
    let res: WrappedBalance = e.terminate(&users.eve, lockup_index).unwrap_json();
    assert_eq!(res.0, amount);

    let terminator_balance = e.ft_balance_of(&users.eve);
    assert_eq!(terminator_balance, amount);

    // Checking lockup

    // after ALL the schedules have finished

    let lockups = e.get_account_lockups(&users.alice);
    assert!(lockups.is_empty());

    let lockup = e.get_lockup(lockup_index);
    assert_eq!(lockup.total_balance, 0);
    assert_eq!(lockup.claimed_balance, 0);
    assert_eq!(lockup.unclaimed_balance, 0);

    // Trying to claim
    ft_storage_deposit(&users.alice, TOKEN_ID, &users.alice.account_id);
    let res: WrappedBalance = e.claim(&users.alice).unwrap_json();
    assert_eq!(res.0, 0);

    let balance = e.ft_balance_of(&users.alice);
    assert_eq!(balance, 0);
}

#[test]
fn test_lockup_terminate_custom_vesting_before_release() {
    let e = Env::init(None);
    let users = Users::init(&e);
    let amount = d(60000, TOKEN_DECIMALS);
    e.set_time_sec(GENESIS_TIMESTAMP_SEC);
    let lockups = e.get_account_lockups(&users.alice);
    assert!(lockups.is_empty());

    let (lockup_schedule, vesting_schedule) = lockup_vesting_schedule(amount);
    let lockup = Lockup {
        account_id: users.alice.valid_account_id(),
        schedule: lockup_schedule,
        claimed_balance: 0,
        termination_config: Some(TerminationConfig {
            terminator_id: users.eve.valid_account_id(),
            vesting_schedule: Some(HashOrSchedule::Schedule(vesting_schedule)),
        }),
    };

    let balance: WrappedBalance = e.add_lockup(&e.owner, amount, &lockup).unwrap_json();
    assert_eq!(balance.0, amount);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups.len(), 1);
    let lockup_index = lockups[0].0;

    // 1Y, 1 / 4 vested, 0 unlocked
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.total_balance, amount);
    assert_eq!(lockups[0].1.claimed_balance, 0);
    assert_eq!(lockups[0].1.unclaimed_balance, 0);

    // TERMINATE
    ft_storage_deposit(&users.eve, TOKEN_ID, &users.eve.account_id);
    let res: WrappedBalance = e.terminate(&users.eve, lockup_index).unwrap_json();
    assert_eq!(res.0, amount * 3 / 4);
    let terminator_balance = e.ft_balance_of(&users.eve);
    assert_eq!(terminator_balance, amount * 3 / 4);

    // Checking lockup
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.total_balance, amount / 4);
    assert_eq!(lockups[0].1.claimed_balance, 0);
    assert_eq!(lockups[0].1.unclaimed_balance, 0);

    // Trying to claim
    ft_storage_deposit(&users.alice, TOKEN_ID, &users.alice.account_id);
    let res: WrappedBalance = e.claim(&users.alice).unwrap_json();
    assert_eq!(res.0, 0);

    // Rewind to 2Y + Y/3, 1/8 of original should be unlocked
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 2 + ONE_YEAR_SEC / 3);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.total_balance, amount / 4);
    assert_eq!(lockups[0].1.claimed_balance, 0);
    assert_eq!(lockups[0].1.unclaimed_balance, amount / 8);

    // claiming
    let res: WrappedBalance = e.claim(&users.alice).unwrap_json();
    assert_eq!(res.0, amount / 8);

    // Rewind to 2Y + Y * 2 / 3, 1/4 of original unlock, full vested unlock
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 2 + ONE_YEAR_SEC * 2 / 3);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.total_balance, amount / 4);
    assert_eq!(lockups[0].1.claimed_balance, amount / 8);
    assert_eq!(lockups[0].1.unclaimed_balance, amount / 8);

    // claiming
    let res: WrappedBalance = e.claim(&users.alice).unwrap_json();
    assert_eq!(res.0, amount / 8);

    // Checking lockups
    let lockups = e.get_account_lockups(&users.alice);
    assert!(lockups.is_empty());

    // User lockups are empty
    let lockup = e.get_lockup(lockup_index);
    assert_eq!(lockup.total_balance, amount / 4);
    assert_eq!(lockup.claimed_balance, amount / 4);
    assert_eq!(lockup.unclaimed_balance, 0);
}

#[test]
fn test_lockup_terminate_custom_vesting_during_release() {
    let e = Env::init(None);
    let users = Users::init(&e);
    let amount = d(60000, TOKEN_DECIMALS);
    e.set_time_sec(GENESIS_TIMESTAMP_SEC);
    let lockups = e.get_account_lockups(&users.alice);
    assert!(lockups.is_empty());

    let (lockup_schedule, vesting_schedule) = lockup_vesting_schedule(amount);
    let lockup = Lockup {
        account_id: users.alice.valid_account_id(),
        schedule: lockup_schedule,
        claimed_balance: 0,
        termination_config: Some(TerminationConfig {
            terminator_id: users.eve.valid_account_id(),
            vesting_schedule: Some(HashOrSchedule::Schedule(vesting_schedule)),
        }),
    };

    let balance: WrappedBalance = e.add_lockup(&e.owner, amount, &lockup).unwrap_json();
    assert_eq!(balance.0, amount);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups.len(), 1);
    let lockup_index = lockups[0].0;

    // 2Y + Y / 3, 1/8 unlocked
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 2 + ONE_YEAR_SEC / 3);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.total_balance, amount);
    assert_eq!(lockups[0].1.claimed_balance, 0);
    assert_eq!(lockups[0].1.unclaimed_balance, amount / 8);

    // Trying to claim
    ft_storage_deposit(&users.alice, TOKEN_ID, &users.alice.account_id);
    let res: WrappedBalance = e.claim(&users.alice).unwrap_json();
    assert_eq!(res.0, amount / 8);

    // TERMINATE, 2Y + Y / 2, 5/8 unlocked
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 2 + ONE_YEAR_SEC / 2);
    ft_storage_deposit(&users.eve, TOKEN_ID, &users.eve.account_id);
    let res: WrappedBalance = e.terminate(&users.eve, lockup_index).unwrap_json();
    assert_eq!(res.0, amount * 3 / 8);
    let terminator_balance = e.ft_balance_of(&users.eve);
    assert_eq!(terminator_balance, amount * 3 / 8);

    // Checking lockup
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.total_balance, amount * 5 / 8);
    assert_eq!(lockups[0].1.claimed_balance, amount / 8);
    assert_eq!(lockups[0].1.unclaimed_balance, amount / 16);

    // Rewind to 2Y + Y*2/3, 1/4 of original should be unlocked
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 2 + ONE_YEAR_SEC * 2 / 3);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.total_balance, amount * 5 / 8);
    assert_eq!(lockups[0].1.claimed_balance, amount / 8);
    assert_eq!(lockups[0].1.unclaimed_balance, amount / 8);

    // claiming
    let res: WrappedBalance = e.claim(&users.alice).unwrap_json();
    assert_eq!(res.0, amount / 8);

    // Rewind to 3Y + Y * 2 / 3, 5/8 of original unlock, full vested unlock
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 3 + ONE_YEAR_SEC * 2 / 3);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.total_balance, amount * 5 / 8);
    assert_eq!(lockups[0].1.claimed_balance, amount * 2 / 8);
    assert_eq!(lockups[0].1.unclaimed_balance, amount * 3 / 8);

    // claiming
    let res: WrappedBalance = e.claim(&users.alice).unwrap_json();
    assert_eq!(res.0, amount * 3 / 8);

    // Checking lockups
    let lockups = e.get_account_lockups(&users.alice);
    assert!(lockups.is_empty());

    // User lockups are empty
    let lockup = e.get_lockup(lockup_index);
    assert_eq!(lockup.total_balance, amount * 5 / 8);
    assert_eq!(lockup.claimed_balance, amount * 5 / 8);
    assert_eq!(lockup.unclaimed_balance, 0);
}

#[test]
fn test_lockup_terminate_custom_vesting_during_lockup_cliff() {
    let e = Env::init(None);
    let users = Users::init(&e);
    let amount = d(60000, TOKEN_DECIMALS);
    e.set_time_sec(GENESIS_TIMESTAMP_SEC);
    let lockups = e.get_account_lockups(&users.alice);
    assert!(lockups.is_empty());

    let (lockup_schedule, vesting_schedule) = lockup_vesting_schedule(amount);
    let lockup = Lockup {
        account_id: users.alice.valid_account_id(),
        schedule: lockup_schedule,
        claimed_balance: 0,
        termination_config: Some(TerminationConfig {
            terminator_id: users.eve.valid_account_id(),
            vesting_schedule: Some(HashOrSchedule::Schedule(vesting_schedule)),
        }),
    };

    let balance: WrappedBalance = e.add_lockup(&e.owner, amount, &lockup).unwrap_json();
    assert_eq!(balance.0, amount);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups.len(), 1);
    let lockup_index = lockups[0].0;

    // 2Y + Y * 2 / 3, 1/8 unlocked
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 2 + ONE_YEAR_SEC * 2 / 3);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.total_balance, amount);
    assert_eq!(lockups[0].1.claimed_balance, 0);
    assert_eq!(lockups[0].1.unclaimed_balance, amount / 4);

    // Trying to claim
    ft_storage_deposit(&users.alice, TOKEN_ID, &users.alice.account_id);
    let res: WrappedBalance = e.claim(&users.alice).unwrap_json();
    assert_eq!(res.0, amount / 4);

    // TERMINATE, 3Y + Y / 3, 5/6 unlocked
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 3 + ONE_YEAR_SEC / 3);
    ft_storage_deposit(&users.eve, TOKEN_ID, &users.eve.account_id);
    let res: WrappedBalance = e.terminate(&users.eve, lockup_index).unwrap_json();
    assert_eq!(res.0, amount / 6);
    let terminator_balance = e.ft_balance_of(&users.eve);
    assert_eq!(terminator_balance, amount / 6);

    // Checking lockup
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.total_balance, amount * 5 / 6);
    assert_eq!(lockups[0].1.claimed_balance, amount / 4);
    assert_eq!(lockups[0].1.unclaimed_balance, amount / 4);

    // claiming
    let res: WrappedBalance = e.claim(&users.alice).unwrap_json();
    assert_eq!(res.0, amount / 4);

    // Rewind to 4Y
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 4);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.total_balance, amount * 5 / 6);
    assert_eq!(lockups[0].1.claimed_balance, amount * 1 / 2);
    assert_eq!(lockups[0].1.unclaimed_balance, amount * 1 / 4);

    // Rewind to 4Y + 1, full unlock including part of cliff
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 4 + 1);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.total_balance, amount * 5 / 6);
    assert_eq!(lockups[0].1.claimed_balance, amount * 1 / 2);
    assert_eq!(lockups[0].1.unclaimed_balance, amount * 1 / 3);

    // claiming
    let res: WrappedBalance = e.claim(&users.alice).unwrap_json();
    assert_eq!(res.0, amount * 1 / 3);

    // Checking lockups
    let lockups = e.get_account_lockups(&users.alice);
    assert!(lockups.is_empty());

    // User lockups are empty
    let lockup = e.get_lockup(lockup_index);
    assert_eq!(lockup.total_balance, amount * 5 / 6);
    assert_eq!(lockup.claimed_balance, amount * 5 / 6);
    assert_eq!(lockup.unclaimed_balance, 0);
}

#[test]
fn test_lockup_terminate_custom_vesting_after_vesting_finished() {
    let e = Env::init(None);
    let users = Users::init(&e);
    let amount = d(60000, TOKEN_DECIMALS);
    e.set_time_sec(GENESIS_TIMESTAMP_SEC);
    let lockups = e.get_account_lockups(&users.alice);
    assert!(lockups.is_empty());

    let (lockup_schedule, vesting_schedule) = lockup_vesting_schedule(amount);
    let lockup = Lockup {
        account_id: users.alice.valid_account_id(),
        schedule: lockup_schedule,
        claimed_balance: 0,
        termination_config: Some(TerminationConfig {
            terminator_id: users.eve.valid_account_id(),
            vesting_schedule: Some(HashOrSchedule::Schedule(vesting_schedule)),
        }),
    };

    let balance: WrappedBalance = e.add_lockup(&e.owner, amount, &lockup).unwrap_json();
    assert_eq!(balance.0, amount);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups.len(), 1);
    let lockup_index = lockups[0].0;

    // 2Y + Y * 2 / 3, 1/8 unlocked
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 2 + ONE_YEAR_SEC * 2 / 3);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.total_balance, amount);
    assert_eq!(lockups[0].1.claimed_balance, 0);
    assert_eq!(lockups[0].1.unclaimed_balance, amount / 4);

    // Trying to claim
    ft_storage_deposit(&users.alice, TOKEN_ID, &users.alice.account_id);
    let res: WrappedBalance = e.claim(&users.alice).unwrap_json();
    assert_eq!(res.0, amount / 4);

    // TERMINATE, 4Y, fully vested
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 4);
    ft_storage_deposit(&users.eve, TOKEN_ID, &users.eve.account_id);
    let res: WrappedBalance = e.terminate(&users.eve, lockup_index).unwrap_json();
    assert_eq!(res.0, 0);
    let terminator_balance = e.ft_balance_of(&users.eve);
    assert_eq!(terminator_balance, 0);

    // Checking lockup
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.total_balance, amount);
    assert_eq!(lockups[0].1.claimed_balance, amount / 4);
    assert_eq!(lockups[0].1.unclaimed_balance, amount / 2);

    // claiming
    let res: WrappedBalance = e.claim(&users.alice).unwrap_json();
    assert_eq!(res.0, amount / 2);

    // Rewind to 4Y + 1, full unlock including part of cliff
    e.set_time_sec(GENESIS_TIMESTAMP_SEC + ONE_YEAR_SEC * 4 + 1);
    let lockups = e.get_account_lockups(&users.alice);
    assert_eq!(lockups[0].1.total_balance, amount);
    assert_eq!(lockups[0].1.claimed_balance, amount * 3 / 4);
    assert_eq!(lockups[0].1.unclaimed_balance, amount * 1 / 4);

    // Claiming
    let res: WrappedBalance = e.claim(&users.alice).unwrap_json();
    assert_eq!(res.0, amount * 1 / 4);

    // Checking lockups
    let lockups = e.get_account_lockups(&users.alice);
    assert!(lockups.is_empty());

    // Checking by index
    let lockup = e.get_lockup(lockup_index);
    assert_eq!(lockup.total_balance, amount);
    assert_eq!(lockup.claimed_balance, amount);
    assert_eq!(lockup.unclaimed_balance, 0);
}

#[test]
fn test_deposit_whitelist_get() {
    let e = Env::init(None);
    let users = Users::init(&e);
    let amount = d(1, TOKEN_DECIMALS);
    e.set_time_sec(GENESIS_TIMESTAMP_SEC);
    let lockups = e.get_account_lockups(&users.alice);
    assert!(lockups.is_empty());

    // deposit whitelist has owner by default
    let deposit_whitelist = e.get_deposit_whitelist();
    assert_eq!(deposit_whitelist, vec![e.owner.account_id.clone()]);

    // user from whitelist can create lockups
    let lockup = Lockup {
        account_id: users.alice.valid_account_id(),
        schedule: Schedule(vec![
            Checkpoint {
                timestamp: 0,
                balance: 0,
            },
            Checkpoint {
                timestamp: 1,
                balance: amount,
            },
        ]),
        claimed_balance: 0,
        termination_config: None,
    };
    let balance: WrappedBalance = e.add_lockup(&e.owner, amount, &lockup).unwrap_json();
    assert_eq!(balance.0, amount);
    let lockups = e.get_account_lockups(&users.alice);
    // not increased
    assert_eq!(lockups.len(), 1);

    // user from whitelist can add other users
    let res = e.add_to_deposit_whitelist(&e.owner, &users.eve.valid_account_id());
    assert!(res.is_ok());

    let deposit_whitelist = e.get_deposit_whitelist();
    assert_eq!(
        deposit_whitelist,
        vec![e.owner.account_id.clone(), users.eve.account_id.clone()]
    );

    // user from whiltelist can remove other users
    let res = e.remove_from_deposit_whitelist(&users.eve, &e.owner.valid_account_id());
    assert!(res.is_ok());

    let deposit_whitelist = e.get_deposit_whitelist();
    assert_eq!(deposit_whitelist, vec![users.eve.account_id.clone()]);

    // user not from whitelist cannot add users
    let res = e.add_to_deposit_whitelist(&e.owner, &users.dude.valid_account_id());
    assert!(!res.is_ok());
    assert!(format!("{:?}", res.status()).contains("Not in deposit whitelist"));

    // user not from whitelist cannot remove users
    let res = e.remove_from_deposit_whitelist(&e.owner, &users.eve.valid_account_id());
    assert!(!res.is_ok());
    assert!(format!("{:?}", res.status()).contains("Not in deposit whitelist"));

    // user not in whitelist cannot create lockups
    let res = e.add_lockup(&e.owner, amount, &lockup);
    let balance: WrappedBalance = res.unwrap_json();
    assert_eq!(balance.0, 0);
    assert!(res.logs()[0].contains("Refund"));
    let lockups = e.get_account_lockups(&users.alice);
    // not increased
    assert_eq!(lockups.len(), 1);

    // user from whiltelist can remove itself from the list, even if it's the last user
    let res = e.remove_from_deposit_whitelist(&users.eve, &users.eve.valid_account_id());
    assert!(res.is_ok());
    let deposit_whitelist = e.get_deposit_whitelist();
    assert!(deposit_whitelist.is_empty());
}

#[test]
fn test_get_lockups() {
    let e = Env::init(None);
    let users = Users::init(&e);
    let amount = d(1, TOKEN_DECIMALS);
    e.set_time_sec(GENESIS_TIMESTAMP_SEC);
    let lockups = e.get_account_lockups(&users.alice);
    assert!(lockups.is_empty());

    // create some lockups
    for user in vec![&users.alice, &users.bob, &users.charlie] {
        let balance: WrappedBalance = e
            .add_lockup(
                &e.owner,
                amount,
                &Lockup::new_unlocked(user.account_id().clone(), amount),
            )
            .unwrap_json();
        assert_eq!(balance.0, amount);
    }

    // get_num_lockups
    let num_lockups = e.get_num_lockups();
    assert_eq!(num_lockups, 3);

    // get_lockups by indices
    let res = e.get_lockups(&vec![2, 0]);
    assert_eq!(res.len(), 2);
    assert_eq!(res[0].1.account_id, users.charlie.valid_account_id());
    assert_eq!(res[1].1.account_id, users.alice.valid_account_id());

    // get_lockups_paged from to
    let res = e.get_lockups_paged(Some(1), Some(2));
    assert_eq!(res.len(), 1);
    assert_eq!(res[0].1.account_id, users.bob.valid_account_id());

    // get_lockups_paged from
    let res = e.get_lockups_paged(Some(1), None);
    assert_eq!(res.len(), 2);
    assert_eq!(res[0].1.account_id, users.bob.valid_account_id());
    assert_eq!(res[1].1.account_id, users.charlie.valid_account_id());

    // get_lockups_paged to
    let res = e.get_lockups_paged(None, Some(2));
    assert_eq!(res.len(), 2);
    assert_eq!(res[0].1.account_id, users.alice.valid_account_id());
    assert_eq!(res[1].1.account_id, users.bob.valid_account_id());

    // get_lockups_paged all
    let res = e.get_lockups_paged(None, None);
    assert_eq!(res.len(), 3);
    assert_eq!(res[0].1.account_id, users.alice.valid_account_id());
    assert_eq!(res[1].1.account_id, users.bob.valid_account_id());
    assert_eq!(res[2].1.account_id, users.charlie.valid_account_id());
}

#[test]
fn test_get_token_account_id() {
    let e = Env::init(None);

    let result = e.get_token_account_id();
    assert_eq!(result, e.token.valid_account_id());
}

#[test]
fn test_new_draft_group() {
    let e = Env::init(None);
    let users = Users::init(&e);
    e.set_time_sec(GENESIS_TIMESTAMP_SEC);

    let index = e.new_draft_group(&e.owner);
    assert_eq!(index, 0);
    let index = e.new_draft_group(&e.owner);
    assert_eq!(index, 1);
    let index = e.new_draft_group(&e.owner);
    assert_eq!(index, 2);
}

#[test]
fn test_view_draft_groups() {
    let e = Env::init(None);
    e.set_time_sec(GENESIS_TIMESTAMP_SEC);

    e.new_draft_group(&e.owner);
    e.new_draft_group(&e.owner);
    e.new_draft_group(&e.owner);

    let result = e.get_draft_group(2);
    assert!(result.is_some());
    assert_eq!(result.unwrap().num_drafts, 0);
    let result = e.get_draft_group(3);
    assert!(result.is_none());

    let result = e.get_draft_groups_paged(None, None);
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].0, 0);
    assert_eq!(result[1].0, 1);
    assert_eq!(result[2].0, 2);

    let result = e.get_draft_groups_paged(Some(1), Some(2));
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].0, 1);
    assert_eq!(result[0].1.num_drafts, 0);
}

#[test]
fn test_new_draft() {
    let e = Env::init(None);
    let users = Users::init(&e);
    e.set_time_sec(GENESIS_TIMESTAMP_SEC);

    let amount = d(60000, TOKEN_DECIMALS);
    let lockup = Lockup::new_unlocked(users.alice.account_id, amount);
    let draft_group_id = 0;
    let draft = Draft { draft_group_id, lockup_id: None, lockup };

    let res = e.new_draft(&e.owner, &draft);
    assert!(!res.is_ok());
    assert!(format!("{:?}", res.status()).contains("draft group not found"));

    e.new_draft_group(&e.owner);

    // create draft 0
    let res = e.new_draft(&e.owner, &draft);
    assert!(res.is_ok());
    let res: DraftGroupIndex = res.unwrap_json();
    assert_eq!(res, 0);

    // create draft 1
    let res = e.new_draft(&e.owner, &draft);
    assert!(res.is_ok());
    let res: DraftGroupIndex = res.unwrap_json();
    assert_eq!(res, 1);

    // check draft group
    let res = e.get_draft_group(0).unwrap();
    assert_eq!(res.num_drafts, 2);
    assert_eq!(res.total_amount, amount * 2);
}

#[test]
fn test_fund_draft_group() {
    let e = Env::init(None);
    let users = Users::init(&e);
    e.set_time_sec(GENESIS_TIMESTAMP_SEC);

    let amount = d(60000, TOKEN_DECIMALS);
    let lockup = Lockup::new_unlocked(users.alice.account_id.clone(), amount);
    let draft_group_id = 0;
    let draft = Draft { draft_group_id, lockup_id: None, lockup };

    e.new_draft_group(&e.owner);

    // create draft 0
    let res = e.new_draft(&e.owner, &draft);
    assert!(res.is_ok());
    // create draft 1
    let res = e.new_draft(&e.owner, &draft);
    assert!(res.is_ok());

    ft_storage_deposit(&e.owner, TOKEN_ID, &users.alice.account_id);
    let result = e.ft_transfer(&e.owner, amount * 2, &users.alice);

    // fund with not authorized account
    let res = e.fund_draft_group(&users.alice, amount * 2, 0);
    assert!(res.logs()[0].contains("Refund"));
    let balance: WrappedBalance = res.unwrap_json();
    assert_eq!(balance.0, 0);

    // fund draft group
    let res = e.fund_draft_group(&e.owner, amount * 2, 0);
    let balance: WrappedBalance = res.unwrap_json();
    assert_eq!(balance.0, amount * 2);

    let res = e.get_draft_group(0).unwrap();
    assert_eq!(res.funded, true, "expected draft group to be funded");

    // fund again, should fail
    let res = e.fund_draft_group(&e.owner, amount * 2, 0);
    assert!(res.logs()[0].contains("Refund"));
    let balance: WrappedBalance = res.unwrap_json();
    assert_eq!(balance.0, 0);

    // add draft after funding
    let res = e.new_draft(&e.owner, &draft);
    assert!(!res.is_ok());
    assert!(format!("{:?}", res.status()).contains("group already funded"));
}

#[test]
fn test_convert_draft() {
    let e = Env::init(None);
    let users = Users::init(&e);
    e.set_time_sec(GENESIS_TIMESTAMP_SEC);

    let amount = d(60000, TOKEN_DECIMALS);
    let lockup = Lockup::new_unlocked(users.alice.account_id.clone(), amount);
    let draft_group_id = 0;
    let draft = Draft { draft_group_id, lockup_id: None, lockup };

    e.new_draft_group(&e.owner);

    // create draft 0
    let res = e.new_draft(&e.owner, &draft);
    assert!(res.is_ok());

    // try convert before fund
    let res = e.convert_draft(&users.bob, 0);
    assert!(!res.is_ok());
    assert!(format!("{:?}", res.status()).contains("not funded group"));

    // fund draft group
    let res = e.fund_draft_group(&e.owner, amount, 0);
    let balance: WrappedBalance = res.unwrap_json();
    assert_eq!(balance.0, amount);

    // convert by anonymous
    let res = e.convert_draft(&users.bob, 0);
    assert!(res.is_ok());

    let res = e.get_draft(0).unwrap();
    assert_eq!(res.lockup_id, Some(0), "expected lockup_id to be set");

    // try to convert again
    let res = e.convert_draft(&users.bob, 0);
    assert!(!res.is_ok());
    assert!(format!("{:?}", res.status()).contains("draft already converted"));
}

#[test]
fn test_view_drafts() {
    let e = Env::init(None);
    let users = Users::init(&e);
    e.set_time_sec(GENESIS_TIMESTAMP_SEC);

    let amount = d(60000, TOKEN_DECIMALS);
    let lockup = Lockup::new_unlocked(users.alice.account_id.clone(), amount);
    let draft_group_id = 0;
    let draft = Draft { draft_group_id, lockup_id: None, lockup };

    e.new_draft_group(&e.owner);
    e.new_draft(&e.owner, &draft);
    e.new_draft(&e.owner, &draft);
    e.new_draft(&e.owner, &draft);

    // fund draft group
    let res = e.fund_draft_group(&e.owner, amount * 3, 0);
    let balance: WrappedBalance = res.unwrap_json();
    assert_eq!(balance.0, amount * 3);
    let res = e.convert_draft(&users.bob, 0);
    assert!(res.is_ok());

    let res = e.get_drafts(vec![2, 0]);
    assert_eq!(res.len(), 2);

    assert_eq!(res[0].0, 2);
    let draft = &res[0].1;
    assert_eq!(draft.draft_group_id, 0);
    assert_eq!(draft.lockup_id, None);
    assert_eq!(draft.lockup.total_balance, amount);

    assert_eq!(res[1].0, 0);
    let draft = &res[1].1;
    assert_eq!(draft.draft_group_id, 0);
    assert_eq!(draft.lockup_id, Some(0));
    assert_eq!(draft.lockup.total_balance, amount);
}
