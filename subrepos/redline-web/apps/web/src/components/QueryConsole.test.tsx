import { describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryConsole } from "./QueryConsole";
import { ApiError } from "../api/client";
import type { QueryResult } from "../api/types";

const okResult: QueryResult = {
  columns: ["greeting"],
  rows: [["hello"]],
  rowCount: 1,
  rowsAffected: null,
  elapsedMs: 1.2,
  truncated: false,
};

describe("QueryConsole", () => {
  it("calls the injected run handler with sql + maxRows and renders results", async () => {
    const runQuery = vi.fn().mockResolvedValue(okResult);
    const user = userEvent.setup();

    render(<QueryConsole runQuery={runQuery} initialSql="values ('hello')" />);

    await user.click(screen.getByRole("button", { name: /run/i }));

    await waitFor(() => expect(runQuery).toHaveBeenCalledTimes(1));
    expect(runQuery).toHaveBeenCalledWith({
      sql: "values ('hello')",
      maxRows: 1000,
    });

    expect(await screen.findByText("hello")).toBeInTheDocument();
    expect(screen.getByText(/1 rows/i)).toBeInTheDocument();
  });

  it("runs on Ctrl+Enter from the textarea", async () => {
    const runQuery = vi.fn().mockResolvedValue(okResult);
    const user = userEvent.setup();

    render(<QueryConsole runQuery={runQuery} initialSql="values (1)" />);
    const editor = screen.getByLabelText("SQL editor");
    editor.focus();
    await user.keyboard("{Control>}{Enter}{/Control}");

    await waitFor(() => expect(runQuery).toHaveBeenCalledTimes(1));
  });

  it("surfaces an ApiError in the error banner", async () => {
    const runQuery = vi
      .fn()
      .mockRejectedValue(new ApiError(400, "syntax error", "/api/query"));
    const user = userEvent.setup();

    render(<QueryConsole runQuery={runQuery} initialSql="values (1)" />);
    await user.click(screen.getByRole("button", { name: /run/i }));

    expect(await screen.findByRole("alert")).toHaveTextContent("syntax error");
  });
});
