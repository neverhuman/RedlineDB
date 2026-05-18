use redlinedb_kernel::engine::tx::ConcurrentTxStatus;
use redlinedb_kernel::format::Csn;
use redlinedb_kernel::txn::TxStatusTable;

#[test]
fn snapshot_excludes_transactions_active_when_snapshot_was_taken() {
    let mut table = TxStatusTable::new();
    let tx1 = table.begin();
    let snapshot = table.snapshot();

    table.commit(tx1);

    assert!(!table.is_tx_visible(tx1, &snapshot, None));
    assert!(table.is_tx_visible(tx1, &snapshot, Some(tx1)));
}

#[test]
fn later_snapshot_sees_prior_committed_transaction() {
    let mut table = TxStatusTable::new();
    let tx1 = table.begin();
    table.commit(tx1);

    let snapshot = table.snapshot();

    assert!(table.is_tx_visible(tx1, &snapshot, None));
}

#[test]
fn aborted_transaction_is_never_visible() {
    let mut table = TxStatusTable::new();
    let tx1 = table.begin();
    table.abort(tx1);

    let snapshot = table.snapshot();

    assert!(!table.is_tx_visible(tx1, &snapshot, None));
}

#[test]
fn concurrent_tx_status_frontier_waits_for_contiguous_commits() {
    let txs = ConcurrentTxStatus::new();
    let tx1 = txs.begin();
    let tx2 = txs.begin();
    let csn1 = txs.reserve_commit_csn();
    let csn2 = txs.reserve_commit_csn();

    txs.publish_commit(tx2, csn2);
    assert_eq!(txs.published_csn(), Csn(0));
    assert_eq!(txs.snapshot().visible_csn, Csn(0));

    txs.publish_commit(tx1, csn1);
    assert_eq!(txs.published_csn(), csn2);
    assert_eq!(txs.snapshot().visible_csn, csn2);
}

#[test]
fn concurrent_tx_status_cancelled_csn_allows_frontier_to_advance() {
    let txs = ConcurrentTxStatus::new();
    let tx1 = txs.begin();
    let tx2 = txs.begin();
    let csn1 = txs.reserve_commit_csn();
    let csn2 = txs.reserve_commit_csn();

    txs.publish_commit(tx2, csn2);
    assert_eq!(txs.published_csn(), Csn(0));

    txs.cancel_reserved_csn(csn1);
    txs.abort(tx1);
    assert_eq!(txs.published_csn(), csn2);
    assert!(txs.is_tx_visible(tx2, &txs.snapshot(), None));
    assert!(!txs.is_tx_visible(tx1, &txs.snapshot(), None));
}

#[test]
fn concurrent_tx_status_snapshots_stay_tiny_after_many_commits() {
    let txs = ConcurrentTxStatus::new();
    for _ in 0..1_000 {
        let tx = txs.begin();
        let csn = txs.reserve_commit_csn();
        txs.publish_commit(tx, csn);
    }

    let snapshot = txs.snapshot();
    assert_eq!(snapshot.visible_csn, Csn(1_000));
    assert!(snapshot.active.is_empty());
    assert_eq!(txs.stats().committed_states, 1_000);
}
