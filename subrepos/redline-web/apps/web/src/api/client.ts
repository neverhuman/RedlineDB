import type {
  ConnectionInfo,
  Health,
  MetricsSnapshot,
  QueryRequest,
  QueryResult,
  SchemaResponse,
  SlowQueriesResponse,
  TablePage,
  TablePageOptions,
  TableSchema,
} from "./types";
import { parseMetricsFrame } from "./decode";

/** Typed error thrown for any non-2xx response. `error` carries the server
 * `{ error }` message when present (ApiError DTO), else a generic fallback. */
export class ApiError extends Error {
  readonly status: number;
  readonly url: string;
  constructor(status: number, message: string, url: string) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.url = url;
  }
}

async function parseError(res: Response): Promise<ApiError> {
  let message = `${res.status} ${res.statusText}`.trim();
  try {
    const body = (await res.json()) as unknown;
    if (
      body &&
      typeof body === "object" &&
      "error" in body &&
      typeof (body as { error: unknown }).error === "string"
    ) {
      message = (body as { error: string }).error;
    }
  } catch {
    // Body was not JSON (or empty); keep the status-line fallback.
  }
  return new ApiError(res.status, message, res.url);
}

async function request<T>(input: string, init?: RequestInit): Promise<T> {
  const res = await fetch(input, {
    headers: { Accept: "application/json", ...(init?.headers ?? {}) },
    ...init,
  });
  if (!res.ok) {
    throw await parseError(res);
  }
  return (await res.json()) as T;
}

function buildTableQuery(opts?: TablePageOptions): string {
  const parts: string[] = [];
  const add = (key: string, value: string) =>
    parts.push(`${encodeURIComponent(key)}=${encodeURIComponent(value)}`);
  if (opts?.limit != null) add("limit", String(opts.limit));
  if (opts?.offset != null) add("offset", String(opts.offset));
  if (opts?.orderBy != null && opts.orderBy !== "") add("orderBy", opts.orderBy);
  if (opts?.dir != null) add("dir", opts.dir);
  return parts.length > 0 ? `?${parts.join("&")}` : "";
}

export function getHealth(): Promise<Health> {
  return request<Health>("/api/health");
}

export function getConnection(): Promise<ConnectionInfo> {
  return request<ConnectionInfo>("/api/connection");
}

export function getSchema(): Promise<SchemaResponse> {
  return request<SchemaResponse>("/api/schema");
}

export function getTableSchema(name: string): Promise<TableSchema> {
  return request<TableSchema>(`/api/tables/${encodeURIComponent(name)}/schema`);
}

export function getTablePage(
  name: string,
  opts?: TablePageOptions,
): Promise<TablePage> {
  return request<TablePage>(
    `/api/tables/${encodeURIComponent(name)}${buildTableQuery(opts)}`,
  );
}

export function runQuery(req: QueryRequest): Promise<QueryResult> {
  return request<QueryResult>("/api/query", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(req),
  });
}

export function getMetrics(): Promise<MetricsSnapshot> {
  return request<MetricsSnapshot>("/api/metrics");
}

export function getSlowQueries(limit?: number): Promise<SlowQueriesResponse> {
  const q = limit != null ? `?limit=${encodeURIComponent(String(limit))}` : "";
  return request<SlowQueriesResponse>(`/api/slow-queries${q}`);
}

/** Subscribe to the metrics SSE stream. Returns an unsubscribe function that
 * closes the underlying EventSource. */
export function subscribeMetrics(
  onTick: (snapshot: MetricsSnapshot) => void,
  onError?: (err: Event) => void,
): () => void {
  const source = new EventSource("/api/metrics/stream");
  source.onmessage = (ev: MessageEvent<string>) => {
    try {
      // Validate the untyped frame before narrowing it to a typed snapshot.
      onTick(parseMetricsFrame(ev.data));
    } catch {
      // Ignore malformed frames rather than tearing the stream down.
    }
  };
  source.onerror = (ev: Event) => {
    onError?.(ev);
  };
  return () => source.close();
}

export const apiClient = {
  getHealth,
  getConnection,
  getSchema,
  getTableSchema,
  getTablePage,
  runQuery,
  getMetrics,
  getSlowQueries,
  subscribeMetrics,
};

export type ApiClient = typeof apiClient;
