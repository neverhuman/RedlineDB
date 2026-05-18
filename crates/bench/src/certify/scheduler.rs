#[path = "scheduler/dispatch.rs"]
mod dispatch;
#[path = "scheduler/queue.rs"]
mod queue;
#[path = "scheduler/types.rs"]
mod types;

pub use dispatch::dispatch_parallel;
pub use queue::{available_cores, build_job_queue, dispatch_parallel_with_spawner};
pub use types::{Job, MAX_PARALLEL_THREADS_ENV, RESERVED_CORES, ScheduledOutcome, SchedulerStats};
