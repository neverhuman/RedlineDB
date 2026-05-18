use std::collections::{BTreeSet, HashMap};

use crate::format::ids::{Csn, TxId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Isolation {
    ReadCommitted,
    Snapshot,
    Serializable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TxState {
    InProgress,
    Committed(Csn),
    Aborted,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub visible_csn: Csn,
    pub xmin: TxId,
    pub xmax: TxId,
    pub active: BTreeSet<TxId>,
}

#[derive(Clone, Debug, Default)]
pub struct TxStatusTable {
    states: HashMap<TxId, TxState>,
    next_tx: u64,
    next_csn: u64,
}

impl TxStatusTable {
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
            next_tx: 1,
            next_csn: 1,
        }
    }

    pub fn begin(&mut self) -> TxId {
        let tx = TxId(self.next_tx);
        self.next_tx += 1;
        self.states.insert(tx, TxState::InProgress);
        tx
    }

    pub fn commit(&mut self, tx: TxId) -> Csn {
        let csn = Csn(self.next_csn);
        self.next_csn += 1;
        self.states.insert(tx, TxState::Committed(csn));
        csn
    }

    pub fn abort(&mut self, tx: TxId) {
        self.states.insert(tx, TxState::Aborted);
    }

    pub fn state(&self, tx: TxId) -> TxState {
        self.states.get(&tx).copied().unwrap_or(TxState::Aborted)
    }

    pub fn snapshot(&self) -> Snapshot {
        let active: BTreeSet<TxId> = self
            .states
            .iter()
            .filter_map(|(tx, state)| (*state == TxState::InProgress).then_some(*tx))
            .collect();

        let xmin = active.first().copied().unwrap_or(TxId(self.next_tx));
        Snapshot {
            visible_csn: Csn(self.next_csn.saturating_sub(1)),
            xmin,
            xmax: TxId(self.next_tx),
            active,
        }
    }

    pub fn is_tx_visible(&self, tx: TxId, snapshot: &Snapshot, owner: Option<TxId>) -> bool {
        if Some(tx) == owner {
            return true;
        }

        match self.state(tx) {
            TxState::Committed(csn) => {
                csn <= snapshot.visible_csn && !snapshot.active.contains(&tx)
            }
            TxState::InProgress | TxState::Aborted => false,
        }
    }
}
