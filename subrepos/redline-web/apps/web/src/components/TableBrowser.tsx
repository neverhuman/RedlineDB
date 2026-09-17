import { useEffect, useMemo, useState } from "react";
import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { getTablePage, getTableSchema } from "../api/client";
import type { TablePageOptions } from "../api/types";
import { formatNumber } from "../lib/format";
import { renderCell } from "../lib/format";

interface TableBrowserProps {
  table: string | null;
}

const PAGE_SIZES = [25, 50, 100, 250];

export function TableBrowser({ table }: TableBrowserProps) {
  const [limit, setLimit] = useState(50);
  const [offset, setOffset] = useState(0);
  const [orderBy, setOrderBy] = useState<string | undefined>(undefined);
  const [dir, setDir] = useState<"asc" | "desc">("asc");

  // Reset paging/sort when the selected table changes.
  useEffect(() => {
    setOffset(0);
    setOrderBy(undefined);
    setDir("asc");
  }, [table]);

  const opts: TablePageOptions = useMemo(
    () => ({ limit, offset, orderBy, dir }),
    [limit, offset, orderBy, dir],
  );

  // Narrow `table` once: the queries only fire when it is non-null (`enabled`),
  // so the empty-string default is never sent to the API.
  const tableName = table ?? "";

  const schema = useQuery({
    queryKey: ["table-schema", table],
    queryFn: () => getTableSchema(tableName),
    enabled: table != null,
  });

  const page = useQuery({
    queryKey: ["table-page", table, opts],
    queryFn: () => getTablePage(tableName, opts),
    enabled: table != null,
    placeholderData: keepPreviousData,
  });

  if (table == null) {
    return (
      <div className="browser browser--empty">
        <p className="grid__empty">Select a table from the schema tree.</p>
      </div>
    );
  }

  const data = page.data;
  const total = data?.total ?? null;
  const columns = data?.columns ?? [];
  const rows = data?.rows ?? [];

  const onSort = (col: string) => {
    if (orderBy === col) {
      setDir((d) => (d === "asc" ? "desc" : "asc"));
    } else {
      setOrderBy(col);
      setDir("asc");
    }
    setOffset(0);
  };

  const pageStart = rows.length === 0 ? 0 : offset + 1;
  const pageEnd = offset + rows.length;
  const canPrev = offset > 0;
  const canNext = total != null ? offset + limit < total : rows.length === limit;

  return (
    <div className="browser">
      <div className="browser__bar">
        <div className="browser__title">
          <span className="browser__name">{table}</span>
          {schema.data?.rowCount != null ? (
            <span className="browser__count">
              {formatNumber(schema.data.rowCount)} rows
            </span>
          ) : null}
        </div>

        <div className="browser__pager">
          <label className="browser__limit">
            page size
            <select
              value={limit}
              onChange={(e) => {
                setLimit(Number(e.target.value));
                setOffset(0);
              }}
              aria-label="Page size"
            >
              {PAGE_SIZES.map((n) => (
                <option key={n} value={n}>
                  {n}
                </option>
              ))}
            </select>
          </label>
          <span className="browser__range">
            {total != null
              ? `${formatNumber(pageStart)}–${formatNumber(pageEnd)} of ${formatNumber(total)}`
              : `${formatNumber(pageStart)}–${formatNumber(pageEnd)}`}
          </span>
          <div className="browser__nav">
            <button
              type="button"
              className="btn"
              onClick={() => setOffset(0)}
              disabled={!canPrev}
            >
              «
            </button>
            <button
              type="button"
              className="btn"
              onClick={() => setOffset((o) => Math.max(0, o - limit))}
              disabled={!canPrev}
            >
              ‹ Prev
            </button>
            <button
              type="button"
              className="btn"
              onClick={() => setOffset((o) => o + limit)}
              disabled={!canNext}
            >
              Next ›
            </button>
          </div>
        </div>
      </div>

      {page.isError ? (
        <div className="banner banner--error" role="alert">
          <span className="banner__icon">⚠</span>
          <span className="banner__text">
            {(page.error as Error)?.message ?? "failed to load table"}
          </span>
        </div>
      ) : null}

      <div className="grid">
        <div className="grid__scroll">
          <table className="grid__table">
            <thead>
              <tr>
                <th className="grid__gutter" scope="col">
                  #
                </th>
                {columns.map((col, i) => {
                  const active = orderBy === col;
                  return (
                    <th
                      key={`${col}:${i}`}
                      scope="col"
                      className={`grid__sortable ${active ? "is-sorted" : ""}`}
                      onClick={() => onSort(col)}
                      title={`Sort by ${col}`}
                    >
                      <span>{col}</span>
                      <span className="grid__sort-ind">
                        {active ? (dir === "asc" ? "▲" : "▼") : "⇅"}
                      </span>
                    </th>
                  );
                })}
              </tr>
            </thead>
            <tbody>
              {columns.length > 0 && rows.length === 0 ? (
                <tr>
                  <td className="grid__empty" colSpan={columns.length + 1}>
                    {page.isLoading ? "Loading…" : "Empty table."}
                  </td>
                </tr>
              ) : (
                rows.map((row, r) => (
                  <tr key={r}>
                    <td className="grid__gutter">{offset + r + 1}</td>
                    {columns.map((col, c) => {
                      const cell = renderCell(row[c] ?? null);
                      return (
                        <td
                          key={`${c}:${col}`}
                          className={
                            cell.isNull
                              ? "grid__cell grid__cell--null"
                              : "grid__cell"
                          }
                        >
                          {cell.text}
                        </td>
                      );
                    })}
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}
