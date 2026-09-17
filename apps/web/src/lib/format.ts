import type { CellValue } from "../api/types";

/** Human-readable byte size (binary units). Accepts null/undefined -> "—". */
export function formatBytes(bytes: number | null | undefined): string {
  if (bytes == null || Number.isNaN(bytes)) return "—";
  if (bytes < 0) return `-${formatBytes(-bytes)}`;
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB", "PB", "EB"];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const precision = value >= 100 ? 0 : value >= 10 ? 1 : 2;
  return `${value.toFixed(precision)} ${units[unit]}`;
}

/** Group-separated integer/decimal. null/undefined -> "—". */
export function formatNumber(n: number | null | undefined): string {
  if (n == null || Number.isNaN(n)) return "—";
  return n.toLocaleString("en-US", { maximumFractionDigits: 2 });
}

/** Format an elapsed duration given in milliseconds. */
export function formatMs(ms: number | null | undefined): string {
  if (ms == null || Number.isNaN(ms)) return "—";
  if (ms < 1) return `${(ms * 1000).toFixed(0)} µs`;
  if (ms < 1000) {
    const precision = ms < 10 ? 2 : ms < 100 ? 1 : 0;
    return `${ms.toFixed(precision)} ms`;
  }
  const secs = ms / 1000;
  if (secs < 60) return `${secs.toFixed(2)} s`;
  const mins = Math.floor(secs / 60);
  const rem = secs - mins * 60;
  return `${mins}m ${rem.toFixed(0)}s`;
}

export interface RenderedCell {
  /** Display text for the cell. */
  text: string;
  /** True when the underlying value is SQL NULL (style it distinctly). */
  isNull: boolean;
}

/** Normalize a single result cell into display text plus a NULL flag so the
 * grid can render NULL with a distinct (muted/italic) style. */
export function renderCell(value: CellValue): RenderedCell {
  if (value === null || value === undefined) {
    return { text: "NULL", isNull: true };
  }
  if (typeof value === "boolean") {
    return { text: value ? "true" : "false", isNull: false };
  }
  if (typeof value === "number") {
    // Render raw to preserve fidelity of IDs / precise values in the grid.
    return { text: String(value), isNull: false };
  }
  return { text: value, isNull: false };
}
