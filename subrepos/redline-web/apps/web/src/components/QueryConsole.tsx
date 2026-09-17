import { useCallback, useRef, useState } from "react";
import type * as React from "react";
import { ApiError, apiClient } from "../api/client";
import type { QueryRequest, QueryResult } from "../api/types";
import { formatMs, formatNumber } from "../lib/format";
import { ResultsGrid } from "./ResultsGrid";

interface QueryConsoleProps {
  /** Injectable runner (defaults to the real API client) — eases testing. */
  runQuery?: (req: QueryRequest) => Promise<QueryResult>;
  /** Optional seed SQL. */
  initialSql?: string;
}

const HISTORY_LIMIT = 12;
const DEFAULT_SQL = "values (1, 'hello');";

export function QueryConsole({
  runQuery = apiClient.runQuery,
  initialSql = DEFAULT_SQL,
}: QueryConsoleProps) {
  const [sql, setSql] = useState(initialSql);
  const [maxRows, setMaxRows] = useState(1000);
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<QueryResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [history, setHistory] = useState<string[]>([]);
  const runningRef = useRef(false);

  const run = useCallback(async () => {
    const trimmed = sql.trim();
    if (trimmed === "" || runningRef.current) return;
    runningRef.current = true;
    setRunning(true);
    setError(null);
    try {
      const res = await runQuery({
        sql: trimmed,
        maxRows: Number.isFinite(maxRows) && maxRows > 0 ? maxRows : null,
      });
      setResult(res);
      setHistory((prev) =>
        [trimmed, ...prev.filter((q) => q !== trimmed)].slice(0, HISTORY_LIMIT),
      );
    } catch (e) {
      const msg =
        e instanceof ApiError
          ? e.message
          : e instanceof Error
            ? e.message
            : "query failed";
      setError(msg);
    } finally {
      runningRef.current = false;
      setRunning(false);
    }
  }, [sql, maxRows, runQuery]);

  const onKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
      e.preventDefault();
      void run();
    }
  };

  return (
    <div className="console">
      <div className="console__editor">
        <textarea
          className="console__sql"
          spellCheck={false}
          value={sql}
          onChange={(e) => setSql(e.target.value)}
          onKeyDown={onKeyDown}
          title="Write SQL — ⌘/Ctrl + Enter to run"
          aria-label="SQL editor"
        />
        <div className="console__toolbar">
          <button
            type="button"
            className="btn btn--primary"
            onClick={() => void run()}
            disabled={running || sql.trim() === ""}
          >
            {running ? "Running…" : "Run ⌘⏎"}
          </button>
          <label className="console__maxrows">
            max rows
            <input
              type="number"
              min={1}
              value={maxRows}
              onChange={(e) => setMaxRows(Number(e.target.value))}
              aria-label="Maximum rows"
            />
          </label>

          {result ? (
            <div className="console__stats" role="status">
              <span className="badge badge--rows">
                {formatNumber(result.rowCount)} rows
              </span>
              <span className="badge badge--time">
                {formatMs(result.elapsedMs)}
              </span>
              {result.rowsAffected != null ? (
                <span className="badge badge--affected">
                  {formatNumber(result.rowsAffected)} affected
                </span>
              ) : null}
              {result.truncated ? (
                <span className="badge badge--trunc" title="result hit maxRows cap">
                  truncated
                </span>
              ) : null}
            </div>
          ) : null}
        </div>
      </div>

      {error ? (
        <div className="banner banner--error" role="alert">
          <span className="banner__icon">⚠</span>
          <span className="banner__text">{error}</span>
          <button
            type="button"
            className="banner__close"
            onClick={() => setError(null)}
            aria-label="Dismiss error"
          >
            ×
          </button>
        </div>
      ) : null}

      <div className="console__results">
        {result ? (
          <ResultsGrid columns={result.columns} rows={result.rows} />
        ) : (
          <div className="grid grid--empty">
            <p className="grid__empty">Run a query to see results.</p>
          </div>
        )}
      </div>

      {history.length > 0 ? (
        <div className="console__history">
          <span className="console__history-label">history</span>
          <ul className="console__history-list">
            {history.map((q, i) => (
              <li key={i}>
                <button
                  type="button"
                  className="console__history-item"
                  title={q}
                  onClick={() => setSql(q)}
                >
                  {q.replace(/\s+/g, " ").slice(0, 64)}
                </button>
              </li>
            ))}
          </ul>
        </div>
      ) : null}
    </div>
  );
}
