// Proof-aware runtime decoders for untyped boundary values (SSE frames, raw
// JSON). Every server payload that crosses the network boundary is validated
// here before it is narrowed to a contract DTO — never cast with `as`. A
// malformed value throws `DecodeError` so callers can drop the frame instead
// of trusting an unproven shape.

import type { LatencyMs, MetricsSnapshot, TableSize } from "./types";

export class DecodeError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "DecodeError";
  }
}

function asObject(value: unknown, ctx: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new DecodeError(`${ctx}: expected an object`);
  }
  return value as Record<string, unknown>;
}

function num(obj: Record<string, unknown>, key: string, ctx: string): number {
  const v = obj[key];
  if (typeof v !== "number" || !Number.isFinite(v)) {
    throw new DecodeError(`${ctx}.${key}: expected a finite number`);
  }
  return v;
}

function nullableNum(
  obj: Record<string, unknown>,
  key: string,
  ctx: string,
): number | null {
  const v = obj[key];
  if (v === null) return null;
  if (typeof v !== "number" || !Number.isFinite(v)) {
    throw new DecodeError(`${ctx}.${key}: expected a number or null`);
  }
  return v;
}

function str(obj: Record<string, unknown>, key: string, ctx: string): string {
  const v = obj[key];
  if (typeof v !== "string") {
    throw new DecodeError(`${ctx}.${key}: expected a string`);
  }
  return v;
}

function decodeLatency(value: unknown): LatencyMs {
  const o = asObject(value, "latencyMs");
  return {
    p50: num(o, "p50", "latencyMs"),
    p95: num(o, "p95", "latencyMs"),
    p99: num(o, "p99", "latencyMs"),
    max: num(o, "max", "latencyMs"),
  };
}

function decodeTableSize(value: unknown): TableSize {
  const o = asObject(value, "tables[]");
  return {
    name: str(o, "name", "tables[]"),
    rowCount: nullableNum(o, "rowCount", "tables[]"),
    bytes: nullableNum(o, "bytes", "tables[]"),
  };
}

/** Validate an untyped value and narrow it to a {@link MetricsSnapshot}. */
export function decodeMetricsSnapshot(value: unknown): MetricsSnapshot {
  const o = asObject(value, "MetricsSnapshot");
  const dbObj = asObject(o.db, "MetricsSnapshot.db");
  const tablesRaw = o.tables;
  if (!Array.isArray(tablesRaw)) {
    throw new DecodeError("MetricsSnapshot.tables: expected an array");
  }
  return {
    uptimeSecs: num(o, "uptimeSecs", "MetricsSnapshot"),
    totalQueries: num(o, "totalQueries", "MetricsSnapshot"),
    failedQueries: num(o, "failedQueries", "MetricsSnapshot"),
    qps: num(o, "qps", "MetricsSnapshot"),
    latencyMs: decodeLatency(o.latencyMs),
    db: {
      sizeBytes: num(dbObj, "sizeBytes", "db"),
      pageCount: num(dbObj, "pageCount", "db"),
      pageSize: num(dbObj, "pageSize", "db"),
      freelistCount: num(dbObj, "freelistCount", "db"),
      walBytes: num(dbObj, "walBytes", "db"),
    },
    tables: tablesRaw.map(decodeTableSize),
    atUnixMs: num(o, "atUnixMs", "MetricsSnapshot"),
  };
}

/** Parse an SSE/JSON string and decode it as a {@link MetricsSnapshot}. */
export function parseMetricsFrame(data: string): MetricsSnapshot {
  // `JSON.parse` is typed `any`, which is assignable to the validator's
  // `unknown` parameter without a cast; the validator does the narrowing.
  const parsed: unknown = JSON.parse(data);
  return decodeMetricsSnapshot(parsed);
}
