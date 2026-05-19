use std::collections::VecDeque;
use std::sync::{Arc, Condvar};

pub(crate) enum WakeDecision {
    AcquireFree,
    AcquireReentrant,
    Continue,
}

pub(crate) fn front_is(waiters: &VecDeque<Arc<Condvar>>, cv: &Arc<Condvar>) -> bool {
    waiters.front().is_some_and(|next| Arc::ptr_eq(next, cv))
}

pub(crate) fn ensure_waiter(waiters: &mut VecDeque<Arc<Condvar>>, cv: &Arc<Condvar>) {
    if !waiters.iter().any(|next| Arc::ptr_eq(next, cv)) {
        waiters.push_back(Arc::clone(cv));
    }
}

pub(crate) fn drop_waiter(waiters: &mut VecDeque<Arc<Condvar>>, cv: &Arc<Condvar>) {
    if let Some(pos) = waiters.iter().position(|next| Arc::ptr_eq(next, cv)) {
        waiters.remove(pos);
    }
}

pub(crate) fn notify_front(waiters: &VecDeque<Arc<Condvar>>) {
    if let Some(next) = waiters.front().cloned() {
        next.notify_one();
    }
}

pub(crate) fn release_owner<O: Copy + Eq>(
    owner: &mut Option<O>,
    expected_owner: O,
    waiters: &VecDeque<Arc<Condvar>>,
) -> bool {
    if *owner == Some(expected_owner) {
        *owner = None;
        notify_front(waiters);
    }
    owner.is_none() && waiters.is_empty()
}

pub(crate) fn wake_decision<O: Copy + Eq>(
    waiters: &mut VecDeque<Arc<Condvar>>,
    owner: Option<O>,
    desired_owner: O,
    cv: &Arc<Condvar>,
) -> WakeDecision {
    match owner {
        None => {
            if front_is(waiters, cv) {
                waiters.pop_front();
                WakeDecision::AcquireFree
            } else {
                ensure_waiter(waiters, cv);
                WakeDecision::Continue
            }
        }
        Some(current) if current == desired_owner => {
            drop_waiter(waiters, cv);
            WakeDecision::AcquireReentrant
        }
        Some(_) => WakeDecision::Continue,
    }
}
