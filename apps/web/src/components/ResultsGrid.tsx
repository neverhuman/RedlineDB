import type { CellValue } from "../api/types";
import { renderCell } from "../lib/format";

interface ResultsGridProps {
  columns: string[];
  rows: CellValue[][];
  /** Optional message shown when there are no rows. */
  emptyMessage?: string;
  /** Optional 1-based offset for the row-number gutter (for paged browsing). */
  rowOffset?: number;
}

/** Scrollable result table. Renders columns + rows; NULL cells are styled. */
export function ResultsGrid({
  columns,
  rows,
  emptyMessage = "Query returned 0 rows.",
  rowOffset = 0,
}: ResultsGridProps) {
  if (columns.length === 0) {
    return (
      <div className="grid grid--empty">
        <p className="grid__empty">No columns to display.</p>
      </div>
    );
  }

  return (
    <div className="grid">
      <div className="grid__scroll">
        <table className="grid__table">
          <thead>
            <tr>
              <th className="grid__gutter" scope="col">
                #
              </th>
              {columns.map((col, i) => (
                <th key={`${col}:${i}`} scope="col" title={col}>
                  {col}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {rows.length === 0 ? (
              <tr className="grid__empty-row">
                <td className="grid__empty" colSpan={columns.length + 1}>
                  {emptyMessage}
                </td>
              </tr>
            ) : (
              rows.map((row, r) => (
                <tr key={r}>
                  <td className="grid__gutter">{rowOffset + r + 1}</td>
                  {columns.map((col, c) => {
                    const cell = renderCell(row[c] ?? null);
                    return (
                      <td
                        key={`${c}:${col}`}
                        className={cell.isNull ? "grid__cell grid__cell--null" : "grid__cell"}
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
  );
}
