// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::{change_set::ChangeSet, state_store::StateStore, LibraDB};
use crypto::{hash::SPARSE_MERKLE_PLACEHOLDER_HASH, HashValue};
use std::collections::HashMap;
use tempfile::tempdir;
use types::{
    account_address::{AccountAddress, ADDRESS_LENGTH},
    account_state_blob::AccountStateBlob,
};

fn put_account_state_set(
    db: &DB,
    state_store: &StateStore,
    account_state_set: Vec<(AccountAddress, AccountStateBlob)>,
    version: Version,
    root_hash: HashValue,
) -> HashValue {
    let mut cs = ChangeSet::new();
    let root = state_store
        .put_account_state_sets(
            vec![account_state_set.into_iter().collect::<HashMap<_, _>>()],
            version,
            root_hash,
            &mut cs,
        )
        .unwrap()[0];
    db.write_schemas(cs.batch).unwrap();

    root
}

fn verify_state_in_store(
    state_store: &StateStore,
    address: AccountAddress,
    expected_value: Option<&AccountStateBlob>,
    root: HashValue,
) {
    let (value, _proof) = state_store
        .get_account_state_with_proof_by_state_root(address, root)
        .unwrap();
    assert_eq!(value.as_ref(), expected_value);
}

#[test]
fn test_pruner() {
    let address = AccountAddress::new([1u8; ADDRESS_LENGTH]);
    let value0 = AccountStateBlob::from(vec![0x01]);
    let value1 = AccountStateBlob::from(vec![0x02]);
    let value2 = AccountStateBlob::from(vec![0x03]);
    let root_default = *SPARSE_MERKLE_PLACEHOLDER_HASH;

    let tmp_dir = tempdir().unwrap();
    let db = LibraDB::new(&tmp_dir).db;
    let state_store = &StateStore::new(Arc::clone(&db));
    let pruner = Pruner::new(
        Arc::clone(&db),
        0, /* num_historical_versions_to_keep */
    );

    let root0 = put_account_state_set(
        &db,
        state_store,
        vec![(address, value0.clone())],
        0, /* version */
        root_default,
    );
    let root1 = put_account_state_set(
        &db,
        state_store,
        vec![(address, value1.clone())],
        1, /* version */
        root0,
    );
    let root2 = put_account_state_set(
        &db,
        state_store,
        vec![(address, value2.clone())],
        2, /* version */
        root1,
    );

    // Prune till version=0.
    {
        pruner.wake_and_wait(0 /* latest_version */).unwrap();
        verify_state_in_store(state_store, address, Some(&value0), root0);
        verify_state_in_store(state_store, address, Some(&value1), root1);
        verify_state_in_store(state_store, address, Some(&value2), root2);
    }
    // Prune till version=1.
    {
        pruner.wake_and_wait(1 /* latest_version */).unwrap();
        // root0 is gone.
        assert!(state_store
            .get_account_state_with_proof_by_state_root(address, root0)
            .is_err());
        // root1 is still there.
        verify_state_in_store(state_store, address, Some(&value1), root1);
        verify_state_in_store(state_store, address, Some(&value2), root2);
    }
    // Prune till version=2.
    {
        pruner.wake_and_wait(2 /* latest_version */).unwrap();
        // root1 is gone.
        assert!(state_store
            .get_account_state_with_proof_by_state_root(address, root1)
            .is_err());
        // root2 is still there.
        verify_state_in_store(state_store, address, Some(&value2), root2);
    }
}

#[test]
fn test_worker_quit_eagerly() {
    let address = AccountAddress::new([1u8; ADDRESS_LENGTH]);
    let value0 = AccountStateBlob::from(vec![0x01]);
    let value1 = AccountStateBlob::from(vec![0x02]);
    let value2 = AccountStateBlob::from(vec![0x03]);
    let root_default = *SPARSE_MERKLE_PLACEHOLDER_HASH;

    let tmp_dir = tempdir().unwrap();
    let db = LibraDB::new(&tmp_dir).db;
    let state_store = &StateStore::new(Arc::clone(&db));

    let root0 = put_account_state_set(
        &db,
        state_store,
        vec![(address, value0.clone())],
        0, /* version */
        root_default,
    );
    let root1 = put_account_state_set(
        &db,
        state_store,
        vec![(address, value1.clone())],
        1, /* version */
        root0,
    );
    let root2 = put_account_state_set(
        &db,
        state_store,
        vec![(address, value2.clone())],
        2, /* version */
        root1,
    );

    {
        let (command_sender, command_receiver) = channel();
        let worker = Worker::new(
            Arc::clone(&db),
            command_receiver,
            Arc::new(AtomicU64::new(0)), /* progress */
        );
        command_sender
            .send(Command::Prune {
                least_readable_version: 1,
            })
            .unwrap();
        command_sender
            .send(Command::Prune {
                least_readable_version: 2,
            })
            .unwrap();
        command_sender.send(Command::Quit).unwrap();
        // Worker quits immediately although `Command::Quit` is not the first command sent.
        worker.work_loop();
        verify_state_in_store(state_store, address, Some(&value0), root0);
        verify_state_in_store(state_store, address, Some(&value1), root1);
        verify_state_in_store(state_store, address, Some(&value2), root2);
    }
}
