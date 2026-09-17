import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { ResultsGrid } from "./ResultsGrid";

describe("ResultsGrid", () => {
  it("renders column headers and cell values including styled NULL", () => {
    render(
      <ResultsGrid
        columns={["id", "name", "note"]}
        rows={[
          [1, "alice", null],
          [2, "bob", "hi"],
        ]}
      />,
    );

    expect(screen.getByText("id")).toBeInTheDocument();
    expect(screen.getByText("name")).toBeInTheDocument();
    expect(screen.getByText("alice")).toBeInTheDocument();
    expect(screen.getByText("bob")).toBeInTheDocument();

    const nullCell = screen.getByText("NULL");
    expect(nullCell).toBeInTheDocument();
    expect(nullCell).toHaveClass("grid__cell--null");
  });

  it("shows a 0-row empty state", () => {
    render(<ResultsGrid columns={["id"]} rows={[]} />);
    expect(screen.getByText(/0 rows/i)).toBeInTheDocument();
  });
});
