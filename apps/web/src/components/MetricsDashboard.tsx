import { useMemo } from "react";
import type * as React from "react";
import { useQuery } from "@tanstack/react-query";
import {
  Bar,
  BarChart,
  CartesianGrid,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { getSlowQueries } from "../api/client";
import { useMetricsStream } from "../hooks/useMetricsStream";
import type { MetricsSnapshot, TableSize } from "../api/types";
import { formatBytes, formatMs, formatNumber } from "../lib/format";

const CHART_AXIS = "#5b6478";
const CHART_GRID = "#222838";
const COLORS = {
  qps: "#4cc2ff",
  p50: "#34d399",
  p95: "#fbbf24",
  p99: "#f87171",
  bytes: "#7c8cff",
  rows: "#22b8a6",
};

interface Point {
  t: number;
  label: string;
  qps: number;
  p50: number;
  p95: number;
  p99: number;
}

function toSeries(history: MetricsSnapshot[]): Point[] {
  return history.map((s) => ({
    t: s.atUnixMs,
    label: new Date(s.atUnixMs).toLocaleTimeString("en-US", {
      hour12: false,
    }),
    qps: Number(s.qps.toFixed(2)),
    p50: s.latencyMs.p50,
    p95: s.latencyMs.p95,
    p99: s.latencyMs.p99,
  }));
}

export function MetricsDashboard() {
  const { history, latest, source, connected } = useMetricsStream(60);
  const slow = useQuery({
    queryKey: ["slow-queries", 20],
    queryFn: () => getSlowQueries(20),
    refetchInterval: 5000,
  });

  const series = useMemo(() => toSeries(history), [history]);

  const topTables = useMemo<TableSize[]>(() => {
    const tables = latest?.tables ?? [];
    return [...tables]
      .sort((a, b) => (b.bytes ?? 0) - (a.bytes ?? 0))
      .slice(0, 8);
  }, [latest]);

  return (
    <div className="obs">
      <div className="obs__cards">
        <StatCard
          label="Queries"
          value={formatNumber(latest?.totalQueries)}
          sub={`${formatNumber(latest?.failedQueries ?? 0)} failed`}
        />
        <StatCard
          label="QPS"
          value={latest ? latest.qps.toFixed(2) : "—"}
          sub={`uptime ${formatUptime(latest?.uptimeSecs)}`}
        />
        <StatCard
          label="DB size"
          value={formatBytes(latest?.db.sizeBytes)}
          sub={`${formatNumber(latest?.db.pageCount)} pages`}
        />
        <StatCard
          label="WAL"
          value={formatBytes(latest?.db.walBytes)}
          sub={`page ${formatBytes(latest?.db.pageSize)}`}
        />
        <StatCard
          label="p99 latency"
          value={formatMs(latest?.latencyMs.p99)}
          sub={`max ${formatMs(latest?.latencyMs.max)}`}
        />
      </div>

      <div className="obs__grid">
        <Panel
          title="Throughput (qps)"
          right={
            <span className={`obs__src obs__src--${source}`}>
              {connected ? source : "connecting…"}
            </span>
          }
        >
          <ResponsiveContainer width="100%" height={220}>
            <LineChart data={series} margin={{ top: 8, right: 12, bottom: 0, left: -8 }}>
              <CartesianGrid stroke={CHART_GRID} strokeDasharray="3 3" />
              <XAxis dataKey="label" stroke={CHART_AXIS} fontSize={11} minTickGap={28} />
              <YAxis stroke={CHART_AXIS} fontSize={11} allowDecimals />
              <Tooltip
                contentStyle={tooltipStyle}
                labelStyle={{ color: "#cdd3e0" }}
              />
              <Line
                type="monotone"
                dataKey="qps"
                stroke={COLORS.qps}
                strokeWidth={2}
                dot={false}
                isAnimationActive={false}
                name="qps"
              />
            </LineChart>
          </ResponsiveContainer>
        </Panel>

        <Panel title="Latency (p50 / p95 / p99, ms)">
          <ResponsiveContainer width="100%" height={220}>
            <LineChart data={series} margin={{ top: 8, right: 12, bottom: 0, left: -8 }}>
              <CartesianGrid stroke={CHART_GRID} strokeDasharray="3 3" />
              <XAxis dataKey="label" stroke={CHART_AXIS} fontSize={11} minTickGap={28} />
              <YAxis stroke={CHART_AXIS} fontSize={11} />
              <Tooltip
                contentStyle={tooltipStyle}
                labelStyle={{ color: "#cdd3e0" }}
              />
              <Line type="monotone" dataKey="p50" stroke={COLORS.p50} strokeWidth={2} dot={false} isAnimationActive={false} name="p50" />
              <Line type="monotone" dataKey="p95" stroke={COLORS.p95} strokeWidth={2} dot={false} isAnimationActive={false} name="p95" />
              <Line type="monotone" dataKey="p99" stroke={COLORS.p99} strokeWidth={2} dot={false} isAnimationActive={false} name="p99" />
            </LineChart>
          </ResponsiveContainer>
        </Panel>

        <Panel title="Top tables by size">
          {topTables.length === 0 ? (
            <p className="grid__empty">no table stats</p>
          ) : (
            <ResponsiveContainer width="100%" height={220}>
              <BarChart
                data={topTables}
                layout="vertical"
                margin={{ top: 4, right: 16, bottom: 0, left: 8 }}
              >
                <CartesianGrid stroke={CHART_GRID} strokeDasharray="3 3" horizontal={false} />
                <XAxis
                  type="number"
                  stroke={CHART_AXIS}
                  fontSize={11}
                  tickFormatter={(v: number) => formatBytes(v)}
                />
                <YAxis
                  type="category"
                  dataKey="name"
                  stroke={CHART_AXIS}
                  fontSize={11}
                  width={110}
                />
                <Tooltip
                  contentStyle={tooltipStyle}
                  labelStyle={{ color: "#cdd3e0" }}
                  formatter={(v: number | string) => formatBytes(Number(v))}
                />
                <Bar dataKey="bytes" fill={COLORS.bytes} radius={[0, 3, 3, 0]} name="bytes" />
              </BarChart>
            </ResponsiveContainer>
          )}
        </Panel>

        <Panel title="Top tables by row count">
          {topTables.length === 0 ? (
            <p className="grid__empty">no table stats</p>
          ) : (
            <ResponsiveContainer width="100%" height={220}>
              <BarChart
                data={[...topTables].sort(
                  (a, b) => (b.rowCount ?? 0) - (a.rowCount ?? 0),
                )}
                layout="vertical"
                margin={{ top: 4, right: 16, bottom: 0, left: 8 }}
              >
                <CartesianGrid stroke={CHART_GRID} strokeDasharray="3 3" horizontal={false} />
                <XAxis
                  type="number"
                  stroke={CHART_AXIS}
                  fontSize={11}
                  tickFormatter={(v: number) => formatNumber(v)}
                />
                <YAxis
                  type="category"
                  dataKey="name"
                  stroke={CHART_AXIS}
                  fontSize={11}
                  width={110}
                />
                <Tooltip
                  contentStyle={tooltipStyle}
                  labelStyle={{ color: "#cdd3e0" }}
                  formatter={(v: number | string) => formatNumber(Number(v))}
                />
                <Bar dataKey="rowCount" fill={COLORS.rows} radius={[0, 3, 3, 0]} name="rows" />
              </BarChart>
            </ResponsiveContainer>
          )}
        </Panel>
      </div>

      <Panel title="Slow queries">
        {slow.isLoading ? (
          <p className="grid__empty">loading…</p>
        ) : (slow.data?.queries.length ?? 0) === 0 ? (
          <p className="grid__empty">no slow queries recorded</p>
        ) : (
          <ul className="slowlist">
            {slow.data!.queries.map((q, i) => (
              <li key={i} className={`slowlist__row ${q.ok ? "" : "is-err"}`}>
                <code className="slowlist__sql" title={q.sql}>
                  {q.sql.replace(/\s+/g, " ").slice(0, 120)}
                </code>
                <span className="slowlist__meta">
                  <span className="badge badge--time">{formatMs(q.elapsedMs)}</span>
                  {q.rowCount != null ? (
                    <span className="slowlist__rows">{formatNumber(q.rowCount)} rows</span>
                  ) : null}
                  {!q.ok ? <span className="badge badge--trunc">error</span> : null}
                  <span className="slowlist__when">
                    {new Date(q.atUnixMs).toLocaleTimeString("en-US", { hour12: false })}
                  </span>
                </span>
              </li>
            ))}
          </ul>
        )}
      </Panel>
    </div>
  );
}

const tooltipStyle: React.CSSProperties = {
  background: "#11151f",
  border: "1px solid #2a3145",
  borderRadius: 6,
  fontSize: 12,
};

function formatUptime(secs: number | null | undefined): string {
  if (secs == null) return "—";
  const s = Math.floor(secs);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  if (h > 0) return `${h}h ${m}m`;
  if (m > 0) return `${m}m ${sec}s`;
  return `${sec}s`;
}

interface StatCardProps {
  label: string;
  value: string;
  sub?: string;
}

function StatCard({ label, value, sub }: StatCardProps) {
  return (
    <div className="statcard">
      <span className="statcard__label">{label}</span>
      <span className="statcard__value">{value}</span>
      {sub ? <span className="statcard__sub">{sub}</span> : null}
    </div>
  );
}

interface PanelProps {
  title: string;
  right?: React.ReactNode;
  children: React.ReactNode;
}

function Panel({ title, right, children }: PanelProps) {
  return (
    <section className="panel">
      <div className="panel__head">
        <span className="panel__title">{title}</span>
        {right ?? null}
      </div>
      <div className="panel__body">{children}</div>
    </section>
  );
}
