//! `sqlite3_context` opaque type and accessors.
//!
//! `RldbContext` carries the per-invocation state a UDF needs: pointer back
//! to its owning connection, the registered `user_data`, and the slot for
//! the value the UDF chooses to return (set via `sqlite3_result_*`). For
//! aggregate UDFs, `agg_state` holds the per-group accumulator opaque slot.

use std::os::raw::c_void;
use std::ptr;
use std::sync::Mutex;
use std::{collections::HashMap, os::raw::c_int};

use super::value::{RldbValue, RldbValueInner};
use crate::types::*;

pub type AuxDestructorFn = unsafe extern "C" fn(*mut c_void);

struct AuxData {
    ptr_addr: usize,
    destructor: Option<AuxDestructorFn>,
}

#[allow(non_camel_case_types)]
pub struct RldbContext {
    /// Stored as `usize` so the struct is auto-`Send + Sync`; cast back to
    /// `*mut rldb` only via [`RldbContext::db_handle`].
    pub(crate) db_addr: usize,
    /// Stored as `usize` so the struct is auto-`Send + Sync`; cast back to
    /// `*mut c_void` only via [`RldbContext::user_data_ptr`].
    pub(crate) user_data_addr: usize,
    pub(crate) result: Mutex<RldbValueInner>,
    pub(crate) error: Mutex<Option<String>>,
    pub(crate) error_code: Mutex<Option<i32>>,
    /// Optional aggregate-state pointer addr. Reserved for the aggregate-UDF
    /// wiring follow-up; held here so the public API does not need to change.
    #[allow(dead_code)]
    pub(crate) agg_state_addr: Mutex<usize>,
    #[allow(dead_code)]
    pub(crate) agg_state_size: Mutex<usize>,
    pub(crate) agg_state: Mutex<Option<Box<[u8]>>>,
    aux_data: Mutex<HashMap<c_int, AuxData>>,
}

impl RldbContext {
    pub fn new(db: *mut rldb, user_data: *mut c_void) -> Self {
        Self {
            db_addr: db as usize,
            user_data_addr: user_data as usize,
            result: Mutex::new(RldbValueInner::Null),
            error: Mutex::new(None),
            error_code: Mutex::new(None),
            agg_state_addr: Mutex::new(0),
            agg_state_size: Mutex::new(0),
            agg_state: Mutex::new(None),
            aux_data: Mutex::new(HashMap::new()),
        }
    }

    pub fn db_handle(&self) -> *mut rldb {
        self.db_addr as *mut rldb
    }

    pub fn user_data_ptr(&self) -> *mut c_void {
        self.user_data_addr as *mut c_void
    }

    pub fn take_result(&self) -> RldbValue {
        let inner = std::mem::replace(
            &mut *self.result.lock().expect("ctx poisoned"),
            RldbValueInner::Null,
        );
        RldbValue {
            inner,
            text_cache: std::cell::RefCell::new(None),
        }
    }

    pub fn take_error(&self) -> Option<String> {
        self.error.lock().expect("ctx poisoned").take()
    }

    pub fn take_error_code(&self) -> Option<i32> {
        self.error_code.lock().expect("ctx poisoned").take()
    }
}

impl Drop for RldbContext {
    fn drop(&mut self) {
        if let Ok(mut aux_data) = self.aux_data.lock() {
            for (_, entry) in aux_data.drain() {
                if let Some(destructor) = entry.destructor {
                    // SAFETY: the destructor pointer and payload were supplied
                    // together via sqlite3_set_auxdata for this context.
                    unsafe { destructor(entry.ptr_addr as *mut c_void) };
                }
            }
        }
    }
}

/// # Safety
/// `ctx` must be a non-NULL `*mut RldbContext` from this crate, valid for
/// the call duration only.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_context_db_handle(ctx: *mut RldbContext) -> *mut rldb {
    if ctx.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: caller obligation 1 — non-null per the # Safety contract;
    // shared borrow valid for the call duration only; db_addr is a usize
    // form of the pointer the registrar originally provided.
    unsafe { (*ctx).db_handle() }
}

/// # Safety
/// `ctx` must be a non-NULL `*mut RldbContext` from this crate, valid for
/// the call duration only.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_user_data(ctx: *mut RldbContext) -> *mut c_void {
    if ctx.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: caller obligation 1 — non-null per the # Safety contract;
    // shared borrow valid for the call duration only; user_data_addr is a
    // usize form of the pointer the registrar originally provided.
    unsafe { (*ctx).user_data_ptr() }
}

/// # Safety
/// `ctx` must be a non-NULL `*mut RldbContext` from this crate, valid for
/// the call duration only.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_aggregate_context(
    ctx: *mut RldbContext,
    nbytes: c_int,
) -> *mut c_void {
    if ctx.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: caller obligation 1 — non-null per the # Safety contract.
    let ctx = unsafe { &*ctx };
    let mut state = match ctx.agg_state.lock() {
        Ok(state) => state,
        Err(_) => return ptr::null_mut(),
    };
    if state.is_none() {
        if nbytes <= 0 {
            return ptr::null_mut();
        }
        *state = Some(vec![0; nbytes as usize].into_boxed_slice());
        if let Ok(mut addr) = ctx.agg_state_addr.lock() {
            *addr = state.as_mut().unwrap().as_mut_ptr() as usize;
        }
        if let Ok(mut size) = ctx.agg_state_size.lock() {
            *size = nbytes as usize;
        }
    }
    state
        .as_mut()
        .map(|bytes| bytes.as_mut_ptr() as *mut c_void)
        .unwrap_or(ptr::null_mut())
}

/// # Safety
/// `ctx` must be a non-NULL `*mut RldbContext` from this crate, valid for
/// the call duration only.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_get_auxdata(ctx: *mut RldbContext, index: c_int) -> *mut c_void {
    if ctx.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: caller obligation 1 — non-null per the # Safety contract.
    let ctx = unsafe { &*ctx };
    ctx.aux_data
        .lock()
        .ok()
        .and_then(|aux_data| {
            aux_data
                .get(&index)
                .map(|entry| entry.ptr_addr as *mut c_void)
        })
        .unwrap_or(ptr::null_mut())
}

/// # Safety
/// `ctx` must be a non-NULL `*mut RldbContext` from this crate, valid for
/// the call duration only. `data` and `destructor` follow SQLite's auxdata
/// ownership contract for UDF callbacks.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_set_auxdata(
    ctx: *mut RldbContext,
    index: c_int,
    data: *mut c_void,
    destructor: Option<AuxDestructorFn>,
) {
    if ctx.is_null() {
        if let Some(destructor) = destructor {
            // SAFETY: SQLite calls the supplied destructor when auxdata
            // cannot be retained.
            unsafe { destructor(data) };
        }
        return;
    }
    // SAFETY: caller obligation 1 — non-null per the # Safety contract.
    let ctx = unsafe { &*ctx };
    let Ok(mut aux_data) = ctx.aux_data.lock() else {
        if let Some(destructor) = destructor {
            // SAFETY: SQLite calls the supplied destructor when auxdata
            // cannot be retained.
            unsafe { destructor(data) };
        }
        return;
    };
    if let Some(previous) = aux_data.remove(&index) {
        if let Some(destructor) = previous.destructor {
            // SAFETY: paired with the previous sqlite3_set_auxdata call.
            unsafe { destructor(previous.ptr_addr as *mut c_void) };
        }
    }
    if data.is_null() {
        return;
    }
    aux_data.insert(
        index,
        AuxData {
            ptr_addr: data as usize,
            destructor,
        },
    );
}
