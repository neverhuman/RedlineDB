import { useState } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ConnectionBar } from "./components/ConnectionBar";
import { SchemaTree } from "./components/SchemaTree";
import { QueryConsole } from "./components/QueryConsole";
import { TableBrowser } from "./components/TableBrowser";
import { MetricsDashboard } from "./components/MetricsDashboard";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      refetchOnWindowFocus: false,
      staleTime: 2000,
      retry: 1,
    },
  },
});

type Tab = "query" | "browse" | "observability";

const TABS: { id: Tab; label: string }[] = [
  { id: "query", label: "Query" },
  { id: "browse", label: "Browse" },
  { id: "observability", label: "Observability" },
];

function Workbench() {
  const [tab, setTab] = useState<Tab>("query");
  const [selected, setSelected] = useState<string | null>(null);

  const onSelectTable = (name: string) => {
    setSelected(name);
    setTab("browse");
  };

  return (
    <div className="app">
      <ConnectionBar />
      <div className="app__body">
        <SchemaTree selected={selected} onSelect={onSelectTable} />
        <main className="app__main">
          <nav className="tabs" role="tablist">
            {TABS.map((t) => (
              <button
                key={t.id}
                type="button"
                role="tab"
                aria-selected={tab === t.id}
                className={`tabs__tab ${tab === t.id ? "is-active" : ""}`}
                onClick={() => setTab(t.id)}
              >
                {t.label}
              </button>
            ))}
            {selected ? (
              <span className="tabs__context" title={selected}>
                {selected}
              </span>
            ) : null}
          </nav>

          <div className="app__panel">
            {tab === "query" ? (
              <QueryConsole />
            ) : tab === "browse" ? (
              <TableBrowser table={selected} />
            ) : (
              <MetricsDashboard />
            )}
          </div>
        </main>
      </div>
    </div>
  );
}

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <Workbench />
    </QueryClientProvider>
  );
}
