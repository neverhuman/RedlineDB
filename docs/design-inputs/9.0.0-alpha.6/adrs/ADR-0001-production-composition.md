# ADR-0001: production compute/storage composition

Status: accepted for the compute/storage canary  
Supersedes: conflicting daemon and composition proposals in the three custodied merger inputs

The production composition has two shipped service identities: `jainhub` and native `jainnode`.
`jain-fabric` is the dependency-free contract authority. `jain-platform` owns separately signed
training, inference, research, coding, and maintenance adapters. SmartCluster schedules and fences
all executable work. Engine repositories and SmartCluster do not link one another; composition is
performed only by signed adapters and typed contracts.

Legacy protocol compatibility remains an explicit separate negotiation. It cannot advertise or
execute future-major WorkUnits. There is no inline-compute fallback, fake worker success, arbitrary
host execution, Git-worktree isolation, Docker socket, or DinD path.

This ADR narrows implementation ownership only. It does not claim that the binaries, adapters,
protocol, packages, or operational evidence already exist.

