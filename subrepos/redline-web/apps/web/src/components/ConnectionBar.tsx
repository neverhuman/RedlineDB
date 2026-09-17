import { useQuery } from "@tanstack/react-query";
import { getConnection, getHealth } from "../api/client";
import { formatBytes } from "../lib/format";

/** Top status bar: engine/mode, file path, version, read-only and live state. */
export function ConnectionBar() {
  const connection = useQuery({
    queryKey: ["connection"],
    queryFn: getConnection,
  });
  const health = useQuery({
    queryKey: ["health"],
    queryFn: getHealth,
    refetchInterval: 5000,
  });

  const conn = connection.data;
  const healthy = health.data?.status === "ok";
  const engine = conn?.engine ?? health.data?.engine;

  return (
    <header className="connbar">
      <div className="connbar__brand">
        <span className="connbar__logo">▰</span>
        <span className="connbar__title">redline-web</span>
      </div>

      <div className="connbar__meta">
        {engine ? (
          <span className={`badge badge--engine badge--${engine}`}>
            {engine}
          </span>
        ) : null}
        {conn ? (
          <span className="badge badge--mode">{conn.mode}</span>
        ) : null}
        {conn?.readOnly ? (
          <span className="badge badge--ro">read-only</span>
        ) : null}
        {conn ? (
          <span className="connbar__path" title={conn.path}>
            {conn.path}
          </span>
        ) : connection.isLoading ? (
          <span className="connbar__path connbar__path--muted">
            connecting…
          </span>
        ) : (
          <span className="connbar__path connbar__path--err">
            connection unavailable
          </span>
        )}
      </div>

      <div className="connbar__stats">
        {conn ? (
          <>
            <span className="connbar__stat">
              <span className="connbar__stat-k">size</span>
              <span className="connbar__stat-v">
                {formatBytes(conn.sizeBytes)}
              </span>
            </span>
            <span className="connbar__stat">
              <span className="connbar__stat-k">sqlite</span>
              <span className="connbar__stat-v">{conn.sqliteVersion}</span>
            </span>
            {conn.engineVersion ? (
              <span className="connbar__stat">
                <span className="connbar__stat-k">engine</span>
                <span className="connbar__stat-v">{conn.engineVersion}</span>
              </span>
            ) : null}
          </>
        ) : null}
        <span
          className={`connbar__health ${healthy ? "is-ok" : "is-down"}`}
          title={healthy ? "server healthy" : "server unreachable"}
        >
          <span className="dot" />
          {healthy ? "live" : "down"}
        </span>
      </div>
    </header>
  );
}
