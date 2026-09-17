import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError, getSlowQueries, getTablePage, runQuery } from "./client";
import type { QueryResult } from "./types";

function jsonResponse(body: unknown, init?: { status?: number; ok?: boolean }) {
  const status = init?.status ?? 200;
  return {
    ok: init?.ok ?? status < 400,
    status,
    statusText: "",
    url: "/api/query",
    json: () => Promise.resolve(body),
  } as unknown as Response;
}

describe("client.runQuery", () => {
  const fetchMock = vi.fn();

  beforeEach(() => {
    vi.stubGlobal("fetch", fetchMock);
    fetchMock.mockReset();
  });
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("POSTs to /api/query and parses the QueryResult", async () => {
    const result: QueryResult = {
      columns: ["n"],
      rows: [[1]],
      rowCount: 1,
      rowsAffected: null,
      elapsedMs: 0.4,
      truncated: false,
    };
    fetchMock.mockResolvedValueOnce(jsonResponse(result));

    const out = await runQuery({ sql: "select 1 as n", maxRows: 100 });

    expect(out).toEqual(result);
    expect(fetchMock).toHaveBeenCalledTimes(1);
    const [url, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(url).toBe("/api/query");
    expect(init.method).toBe("POST");
    expect(JSON.parse(init.body as string)).toEqual({
      sql: "select 1 as n",
      maxRows: 100,
    });
    expect(
      (init.headers as Record<string, string>)["Content-Type"],
    ).toBe("application/json");
  });

  it("throws a typed ApiError parsing {error} on 400", async () => {
    fetchMock.mockResolvedValueOnce(
      jsonResponse({ error: "near \"slect\": syntax error" }, { status: 400 }),
    );

    const err = await runQuery({ sql: "slect 1", maxRows: null }).catch(
      (e: unknown) => e,
    );

    expect(err).toBeInstanceOf(ApiError);
    expect((err as ApiError).status).toBe(400);
    expect((err as ApiError).message).toBe('near "slect": syntax error');
  });

  it("falls back to the status line when the error body is not JSON", async () => {
    fetchMock.mockResolvedValueOnce({
      ok: false,
      status: 500,
      statusText: "Internal Server Error",
      url: "/api/query",
      json: () => Promise.reject(new Error("not json")),
    } as unknown as Response);

    const err = await runQuery({ sql: "select 1", maxRows: null }).catch(
      (e: unknown) => e,
    );

    expect(err).toBeInstanceOf(ApiError);
    expect((err as ApiError).message).toBe("500 Internal Server Error");
  });
});

describe("client query-string builders", () => {
  const fetchMock = vi.fn();
  beforeEach(() => {
    vi.stubGlobal("fetch", fetchMock);
    fetchMock.mockReset();
    fetchMock.mockResolvedValue(jsonResponse({}));
  });
  afterEach(() => vi.unstubAllGlobals());

  it("encodes table paging options", async () => {
    await getTablePage("users", {
      limit: 25,
      offset: 50,
      orderBy: "id",
      dir: "desc",
    });
    const [url] = fetchMock.mock.calls[0] as [string];
    expect(url).toBe("/api/tables/users?limit=25&offset=50&orderBy=id&dir=desc");
  });

  it("omits empty paging params and encodes slow-query limit", async () => {
    await getTablePage("users");
    expect((fetchMock.mock.calls[0] as [string])[0]).toBe("/api/tables/users");

    await getSlowQueries(10);
    expect((fetchMock.mock.calls[1] as [string])[0]).toBe(
      "/api/slow-queries?limit=10",
    );
  });
});
