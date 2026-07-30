# Native format transitions

There is no ambient or startup migration runner. A physical-format transition
must be an explicit Rust implementation with version checks, crash-boundary
tests, a verified backup prerequisite, and a documented rollback. Unknown,
partial, or newer formats fail closed before mutation.
