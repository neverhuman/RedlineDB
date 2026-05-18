//! Deterministic failpoint registry for the RedlineDB kernel.
//!
//! When the `failpoints` feature is enabled, this module forwards to the
//! [`fail`](https://docs.rs/fail) crate so that tests and benchmarks can
//! configure injection of `panic`, `return(value)`, `off`, `sleep(N)`,
//! `pause`, `print`, and `yield` actions at named points in kernel code.
//!
//! When the feature is disabled, every entry point is an inline no-op so the
//! release build pays zero runtime cost. Lane D owns this scaffolding only;
//! Lane E wires the actual `fail_point!` invocations into hot paths.

pub mod macros;

#[cfg(feature = "failpoints")]
use std::sync::Once;

#[cfg(feature = "failpoints")]
static INIT: Once = Once::new();

/// Initialise the failpoint scenario for the lifetime of the process.
///
/// `fail::FailScenario::setup` returns a guard whose `Drop` impl clears every
/// configured failpoint, which is the correct semantics for a single test
/// case but the wrong semantics for a long-lived embedded engine. We call it
/// exactly once and `mem::forget` the guard so the registry stays armed and
/// callers may then `cfg` failpoints freely from anywhere in the program.
///
/// Calling `init` more than once is safe; the [`Once`] guard makes subsequent
/// calls inert. Without the `failpoints` feature this is a statically
/// inlined no-op.
#[cfg(feature = "failpoints")]
pub fn init() {
    INIT.call_once(|| {
        let scenario = fail::FailScenario::setup();
        std::mem::forget(scenario);
    });
}

/// Initialise the failpoint scenario (no-op when feature is disabled).
#[cfg(not(feature = "failpoints"))]
pub fn init() {}

/// Configure a named failpoint with one of the registry actions.
///
/// Accepted actions match the `fail` crate grammar: `panic`, `return`,
/// `return(value)`, `off`, `print`, `print(msg)`, `pause`, `sleep(N)`,
/// `yield`, plus optional `K%action` (frequency) and `K*action` (count)
/// prefixes which compose with each other (e.g. `25%5*panic`).
///
/// We pre-validate `action` against the same grammar before forwarding
/// to `fail::cfg`. Without this, unknown tokens like `abort` are
/// silently rejected by the `fail` crate and the bench harness happily
/// reports a false pass when `acked == recovered == 0`. Fail-loud at
/// the configuration boundary so the bench-runner sees the error
/// immediately and the per-case json report records the misconfigured
/// action instead of an empty success.
#[cfg(feature = "failpoints")]
pub fn cfg(name: &str, action: &str) -> Result<(), String> {
    validate_action(action)?;
    fail::cfg(name, action)
}

/// Configure a named failpoint (no-op when feature is disabled).
#[cfg(not(feature = "failpoints"))]
pub fn cfg(_name: &str, _action: &str) -> Result<(), String> {
    Ok(())
}

/// Configure `name` so that the first `skip_hits` invocations are
/// no-ops and the very next one panics, killing the process.
///
/// This is the building block the bench failpoint-matrix runner uses
/// to implement `kill_after_n_hits > 1`. The fail crate's `K*action`
/// grammar means "apply action up to K times" — i.e. `5*panic` panics
/// on the FIRST hit and stays armed for 4 more — which is the wrong
/// semantic for "let the workload survive K-1 hits then crash on the
/// K-th". A counted-callback failpoint expresses the right semantic
/// without needing to extend the fail grammar.
///
/// Internally we register a `cfg_callback` whose closure increments
/// an atomic counter and panics once the counter passes `skip_hits`.
/// The callback's owning `Arc` is leaked into the registry the way
/// `fail::cfg_callback` does normally — there is no need for a
/// separate cleanup path because the failpoint is armed for the
/// lifetime of the child process.
///
/// `skip_hits == 0` is equivalent to `cfg(name, "panic")` (panics on
/// the first hit). Callers should prefer the string form when they
/// don't need a count.
#[cfg(feature = "failpoints")]
pub fn cfg_skip_then_panic(name: &str, skip_hits: usize) -> Result<(), String> {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let counter = Arc::new(AtomicUsize::new(0));
    let counter_for_closure = Arc::clone(&counter);
    let skip = skip_hits;
    let name_for_panic = name.to_owned();
    fail::cfg_callback(name, move || {
        let current = counter_for_closure.fetch_add(1, Ordering::SeqCst);
        if current >= skip {
            panic!("failpoint {name_for_panic:?} fired (count={current}, skip={skip})");
        }
    })
}

/// Counted-skip variant (no-op when feature is disabled).
#[cfg(not(feature = "failpoints"))]
pub fn cfg_skip_then_panic(_name: &str, _skip_hits: usize) -> Result<(), String> {
    Ok(())
}

/// Validate an action string against the `fail` 0.5.x grammar.
///
/// Grammar (matching `fail::Action::from_str`):
///
/// ```text
/// action := [freq% ][count* ]task[(args)]
/// task   := off | return | sleep | panic | print | pause | yield | delay
/// ```
///
/// Returns `Err` with a message naming the offending token when the
/// task keyword is unrecognised, the optional frequency / count prefix
/// fails to parse, or parentheses are unbalanced. The error string
/// includes the unsupported token verbatim so callers (e.g. the bench
/// failpoint matrix runner) can surface it in the per-case report.
#[cfg(feature = "failpoints")]
fn validate_action(action: &str) -> Result<(), String> {
    let trimmed = action.trim();
    if trimmed.is_empty() {
        return Err("empty failpoint action".to_owned());
    }

    // Strip optional `(args)` suffix. The fail crate accepts `task(arg)`
    // for `return`, `sleep`, `print`, and `delay`. We do not validate
    // the args content here: that mirrors the fail crate, which only
    // re-parses args lazily inside the matching task arm.
    let mut remain = trimmed;
    if let Some(open) = remain.find('(') {
        let suffix = &remain[open..];
        if !suffix.ends_with(')') {
            return Err(format!(
                "failpoint action {action:?} has unbalanced parentheses"
            ));
        }
        remain = &remain[..open];
    }

    // Strip optional `freq%` prefix. Frequency must parse as f32.
    if let Some(percent) = remain.find('%') {
        let (head, rest) = remain.split_at(percent);
        let rest = &rest[1..]; // skip the `%`
        head.parse::<f32>().map_err(|err| {
            format!("failpoint action {action:?} has invalid frequency {head:?}: {err}")
        })?;
        remain = rest;
    }

    // Strip optional `count*` prefix. Count must parse as usize.
    if let Some(star) = remain.find('*') {
        let (head, rest) = remain.split_at(star);
        let rest = &rest[1..]; // skip the `*`
        head.parse::<usize>().map_err(|err| {
            format!("failpoint action {action:?} has invalid count {head:?}: {err}")
        })?;
        remain = rest;
    }

    let task = remain.trim();
    match task {
        "off" | "return" | "sleep" | "panic" | "print" | "pause" | "yield" | "delay" => Ok(()),
        other => Err(format!(
            "failpoint action {action:?} has unsupported task token {other:?}; \
             expected one of off|return|sleep|panic|print|pause|yield|delay"
        )),
    }
}

#[cfg(all(test, feature = "failpoints"))]
mod tests {
    use super::validate_action;

    #[test]
    fn validate_action_accepts_known_tasks() {
        for action in [
            "panic",
            "return",
            "return(value)",
            "off",
            "print",
            "print(hello)",
            "pause",
            "sleep(10)",
            "yield",
            "delay(5)",
        ] {
            validate_action(action).unwrap_or_else(|err| {
                panic!("expected {action:?} to validate: {err}");
            });
        }
    }

    #[test]
    fn validate_action_accepts_count_and_frequency_prefixes() {
        validate_action("5*panic").expect("count prefix");
        validate_action("50%panic").expect("frequency prefix");
        validate_action("50%5*panic").expect("frequency+count prefix");
    }

    #[test]
    fn validate_action_rejects_abort() {
        let err = validate_action("abort").expect_err("abort must be rejected");
        assert!(
            err.contains("abort"),
            "error must name the bad token: {err}"
        );
    }

    #[test]
    fn validate_action_rejects_empty() {
        validate_action("").expect_err("empty is rejected");
        validate_action("   ").expect_err("blank is rejected");
    }

    #[test]
    fn validate_action_rejects_bad_count() {
        let err = validate_action("xx*panic").expect_err("non-numeric count");
        assert!(err.contains("count"), "error must mention count: {err}");
    }

    #[test]
    fn validate_action_rejects_unbalanced_parens() {
        let err = validate_action("sleep(10").expect_err("unbalanced parens");
        assert!(err.contains("parentheses"), "error mentions parens: {err}");
    }
}
