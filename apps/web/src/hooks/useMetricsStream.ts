import { useEffect, useRef, useState } from "react";
import { getMetrics, subscribeMetrics } from "../api/client";
import type { MetricsSnapshot } from "../api/types";

const DEFAULT_MAX_POINTS = 60;
const POLL_INTERVAL_MS = 2000;

export type StreamSource = "sse" | "polling" | "connecting";

export interface MetricsStreamState {
  /** Most recent snapshot, or null before the first tick. */
  latest: MetricsSnapshot | null;
  /** Rolling window of snapshots (oldest first) for charting. */
  history: MetricsSnapshot[];
  /** Where the current data is coming from. */
  source: StreamSource;
  /** True once at least one snapshot has arrived. */
  connected: boolean;
}

/**
 * Subscribe to the metrics SSE stream, keeping a rolling window (~60 points)
 * suitable for time-series charts. If the EventSource errors, transparently
 * fall back to polling `GET /api/metrics`.
 */
export function useMetricsStream(
  maxPoints: number = DEFAULT_MAX_POINTS,
): MetricsStreamState {
  const [history, setHistory] = useState<MetricsSnapshot[]>([]);
  const [source, setSource] = useState<StreamSource>("connecting");
  const seenRef = useRef<Set<number>>(new Set());

  useEffect(() => {
    let cancelled = false;
    let unsubscribe: (() => void) | null = null;
    let pollTimer: ReturnType<typeof setInterval> | null = null;

    const push = (snapshot: MetricsSnapshot) => {
      if (cancelled) return;
      // De-dupe by timestamp so polling + a recovered stream can't double-count.
      if (seenRef.current.has(snapshot.atUnixMs)) return;
      seenRef.current.add(snapshot.atUnixMs);
      setHistory((prev) => {
        const next = [...prev, snapshot];
        if (next.length > maxPoints) {
          const dropped = next.splice(0, next.length - maxPoints);
          for (const d of dropped) seenRef.current.delete(d.atUnixMs);
        }
        return next;
      });
    };

    const startPolling = () => {
      if (pollTimer != null) return;
      setSource("polling");
      const tick = () => {
        getMetrics()
          .then(push)
          .catch(() => {
            /* keep last-known data; try again next interval */
          });
      };
      tick();
      pollTimer = setInterval(tick, POLL_INTERVAL_MS);
    };

    unsubscribe = subscribeMetrics(
      (snapshot) => {
        if (cancelled) return;
        setSource("sse");
        push(snapshot);
      },
      () => {
        // SSE failed — drop the stream and fall back to polling.
        if (cancelled) return;
        unsubscribe?.();
        unsubscribe = null;
        startPolling();
      },
    );

    return () => {
      cancelled = true;
      unsubscribe?.();
      if (pollTimer != null) clearInterval(pollTimer);
    };
  }, [maxPoints]);

  return {
    latest: history.length > 0 ? history[history.length - 1] : null,
    history,
    source,
    connected: history.length > 0,
  };
}
