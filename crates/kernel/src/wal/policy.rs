#![allow(dead_code)]

use crate::format::Lsn;

use super::manager::WalConfig;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WalScheduleContext {
    pub pending_bytes: usize,
    pub pending_records: usize,
    pub flush_gap_bytes: u64,
    pub configured_write_batch_bytes: usize,
    pub configured_delay_us: u64,
    pub configured_max_group_bytes: u64,
}

impl WalScheduleContext {
    pub(crate) fn from_config(config: &WalConfig) -> Self {
        Self {
            pending_bytes: 0,
            pending_records: 0,
            flush_gap_bytes: 0,
            configured_write_batch_bytes: config.wal_write_batch_bytes,
            configured_delay_us: config.group_commit_delay_us,
            configured_max_group_bytes: config.group_commit_max_batch_bytes,
        }
    }

    pub(crate) fn with_pending(
        config: &WalConfig,
        pending_bytes: usize,
        pending_records: usize,
    ) -> Self {
        Self {
            pending_bytes,
            pending_records,
            ..Self::from_config(config)
        }
    }

    pub(crate) fn with_flush_gap(config: &WalConfig, flush_target: Lsn, durable: Lsn) -> Self {
        Self {
            flush_gap_bytes: flush_target.0.saturating_sub(durable.0),
            ..Self::from_config(config)
        }
    }
}

pub(crate) trait WalSchedulePolicy {
    fn write_batch_bytes(ctx: WalScheduleContext) -> usize;
    fn group_commit_delay_us(ctx: WalScheduleContext) -> u64;
    fn resample_flush_target(ctx: WalScheduleContext) -> bool;
    fn drain_batch_bytes(ctx: WalScheduleContext) -> usize;
}

pub(crate) type ActiveWalSchedulePolicy = WalScheduleDefault;

pub(crate) struct WalScheduleDefault;

impl WalSchedulePolicy for WalScheduleDefault {
    fn write_batch_bytes(ctx: WalScheduleContext) -> usize {
        ctx.configured_write_batch_bytes.max(1)
    }

    fn group_commit_delay_us(ctx: WalScheduleContext) -> u64 {
        if ctx.flush_gap_bytes >= ctx.configured_max_group_bytes {
            0
        } else {
            ctx.configured_delay_us
        }
    }

    fn resample_flush_target(_ctx: WalScheduleContext) -> bool {
        true
    }

    fn drain_batch_bytes(ctx: WalScheduleContext) -> usize {
        ctx.configured_write_batch_bytes.max(1)
    }
}

pub(crate) struct WalScheduleTailLatency;

impl WalSchedulePolicy for WalScheduleTailLatency {
    fn write_batch_bytes(ctx: WalScheduleContext) -> usize {
        (ctx.configured_write_batch_bytes / 4).max(64 * 1024)
    }

    fn group_commit_delay_us(ctx: WalScheduleContext) -> u64 {
        if ctx.pending_records <= 1 {
            0
        } else {
            ctx.configured_delay_us.min(50)
        }
    }

    fn resample_flush_target(ctx: WalScheduleContext) -> bool {
        ctx.pending_bytes <= ctx.configured_write_batch_bytes
    }

    fn drain_batch_bytes(ctx: WalScheduleContext) -> usize {
        Self::write_batch_bytes(ctx)
    }
}

pub(crate) struct WalScheduleFanInAdaptive;

impl WalSchedulePolicy for WalScheduleFanInAdaptive {
    fn write_batch_bytes(ctx: WalScheduleContext) -> usize {
        if ctx.pending_bytes >= ctx.configured_write_batch_bytes {
            ctx.pending_bytes
                .min(ctx.configured_write_batch_bytes.saturating_mul(4))
        } else {
            ctx.configured_write_batch_bytes.max(1)
        }
    }

    fn group_commit_delay_us(ctx: WalScheduleContext) -> u64 {
        if ctx.flush_gap_bytes >= ctx.configured_max_group_bytes {
            0
        } else if ctx.pending_records >= 16 {
            ctx.configured_delay_us / 2
        } else {
            ctx.configured_delay_us
        }
    }

    fn resample_flush_target(_ctx: WalScheduleContext) -> bool {
        true
    }

    fn drain_batch_bytes(ctx: WalScheduleContext) -> usize {
        ctx.configured_write_batch_bytes
            .saturating_mul(2)
            .max(ctx.configured_write_batch_bytes)
            .max(1)
    }
}

pub(crate) struct WalScheduleCheckpointFriendly;

impl WalSchedulePolicy for WalScheduleCheckpointFriendly {
    fn write_batch_bytes(ctx: WalScheduleContext) -> usize {
        ctx.configured_write_batch_bytes
            .saturating_mul(2)
            .clamp(1, 16 * 1024 * 1024)
    }

    fn group_commit_delay_us(ctx: WalScheduleContext) -> u64 {
        if ctx.flush_gap_bytes >= ctx.configured_max_group_bytes / 2 {
            0
        } else {
            ctx.configured_delay_us
        }
    }

    fn resample_flush_target(ctx: WalScheduleContext) -> bool {
        ctx.pending_bytes > 0 || ctx.flush_gap_bytes > 0
    }

    fn drain_batch_bytes(ctx: WalScheduleContext) -> usize {
        Self::write_batch_bytes(ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn audit_policy<P: WalSchedulePolicy>() {
        let ctx = WalScheduleContext {
            pending_bytes: 512 * 1024,
            pending_records: 8,
            flush_gap_bytes: 256 * 1024,
            configured_write_batch_bytes: 1024 * 1024,
            configured_delay_us: 200,
            configured_max_group_bytes: 4 * 1024 * 1024,
        };

        assert!(P::write_batch_bytes(ctx) > 0);
        assert!(P::drain_batch_bytes(ctx) > 0);
        assert!(P::group_commit_delay_us(ctx) <= ctx.configured_delay_us);
        let _ = P::resample_flush_target(ctx);
    }

    #[test]
    fn wal_schedule_drop_ins_preserve_basic_invariants() {
        audit_policy::<WalScheduleDefault>();
        audit_policy::<WalScheduleTailLatency>();
        audit_policy::<WalScheduleFanInAdaptive>();
        audit_policy::<WalScheduleCheckpointFriendly>();
    }
}
