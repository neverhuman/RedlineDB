import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { getSchema } from "../api/client";
import type { SchemaObject, SchemaObjectKind } from "../api/types";
import { formatNumber } from "../lib/format";

interface SchemaTreeProps {
  selected: string | null;
  onSelect: (name: string) => void;
}

const KIND_ORDER: SchemaObjectKind[] = ["table", "view", "index", "trigger"];
const KIND_LABEL: Record<SchemaObjectKind, string> = {
  table: "Tables",
  view: "Views",
  index: "Indexes",
  trigger: "Triggers",
};
const KIND_GLYPH: Record<SchemaObjectKind, string> = {
  table: "▦",
  view: "◫",
  index: "⌗",
  trigger: "⚡",
};

/** Left rail: schema objects grouped by kind. Clicking a table selects it. */
export function SchemaTree({ selected, onSelect }: SchemaTreeProps) {
  const { data, isLoading, isError, error } = useQuery({
    queryKey: ["schema"],
    queryFn: getSchema,
  });
  const [filter, setFilter] = useState("");

  const groups = useMemo(() => {
    const objects = data?.objects ?? [];
    const needle = filter.trim().toLowerCase();
    const byKind = new Map<SchemaObjectKind, SchemaObject[]>();
    for (const obj of objects) {
      if (needle && !obj.name.toLowerCase().includes(needle)) continue;
      const list = byKind.get(obj.kind) ?? [];
      list.push(obj);
      byKind.set(obj.kind, list);
    }
    for (const list of byKind.values()) {
      list.sort((a, b) => a.name.localeCompare(b.name));
    }
    return byKind;
  }, [data, filter]);

  const hasAny = KIND_ORDER.some((k) => (groups.get(k)?.length ?? 0) > 0);

  return (
    <aside className="schema">
      <div className="schema__head">
        <span className="schema__title">Schema</span>
        <input
          className="schema__filter"
          type="search"
          title="filter objects"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          aria-label="Filter schema objects"
        />
      </div>

      <div className="schema__body">
        {isLoading ? (
          <p className="schema__hint">loading schema…</p>
        ) : isError ? (
          <p className="schema__hint schema__hint--err">
            {(error as Error)?.message ?? "failed to load schema"}
          </p>
        ) : !hasAny ? (
          <p className="schema__hint">no objects</p>
        ) : (
          KIND_ORDER.map((kind) => {
            const list = groups.get(kind);
            if (!list || list.length === 0) return null;
            return (
              <SchemaGroup
                key={kind}
                kind={kind}
                objects={list}
                selected={selected}
                onSelect={onSelect}
              />
            );
          })
        )}
      </div>
    </aside>
  );
}

interface SchemaGroupProps {
  kind: SchemaObjectKind;
  objects: SchemaObject[];
  selected: string | null;
  onSelect: (name: string) => void;
}

function SchemaGroup({ kind, objects, selected, onSelect }: SchemaGroupProps) {
  const [open, setOpen] = useState(true);
  const selectable = kind === "table" || kind === "view";
  return (
    <section className="schema__group">
      <button
        type="button"
        className="schema__group-head"
        onClick={() => setOpen((o) => !o)}
        aria-expanded={open}
      >
        <span className={`schema__chevron ${open ? "is-open" : ""}`}>▸</span>
        <span className="schema__group-label">{KIND_LABEL[kind]}</span>
        <span className="schema__group-count">{objects.length}</span>
      </button>
      {open ? (
        <ul className="schema__list">
          {objects.map((obj) => {
            const isActive = selectable && obj.name === selected;
            return (
              <li key={`${kind}:${obj.name}`}>
                <button
                  type="button"
                  className={`schema__item ${isActive ? "is-active" : ""} ${
                    selectable ? "" : "is-static"
                  }`}
                  onClick={
                    selectable ? () => onSelect(obj.name) : undefined
                  }
                  disabled={!selectable}
                  title={obj.sql ?? obj.name}
                >
                  <span className="schema__glyph">{KIND_GLYPH[kind]}</span>
                  <span className="schema__name">{obj.name}</span>
                  {obj.rowCount != null ? (
                    <span className="schema__rows">
                      {formatNumber(obj.rowCount)}
                    </span>
                  ) : null}
                </button>
              </li>
            );
          })}
        </ul>
      ) : null}
    </section>
  );
}
