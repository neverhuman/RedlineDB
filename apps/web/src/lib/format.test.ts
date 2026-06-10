import { describe, expect, it } from "vitest";
import { formatBytes, formatMs, formatNumber, renderCell } from "./format";

describe("formatBytes", () => {
  it("renders bytes below 1 KiB as plain bytes", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(1023)).toBe("1023 B");
  });

  it("scales to binary units with sensible precision", () => {
    expect(formatBytes(1024)).toBe("1.00 KB");
    expect(formatBytes(1536)).toBe("1.50 KB");
    expect(formatBytes(1024 * 1024)).toBe("1.00 MB");
    expect(formatBytes(1024 * 1024 * 1024)).toBe("1.00 GB");
  });

  it("drops decimals for large mantissas", () => {
    expect(formatBytes(900 * 1024)).toBe("900 KB");
  });

  it("returns an em-dash for null/NaN", () => {
    expect(formatBytes(null)).toBe("—");
    expect(formatBytes(undefined)).toBe("—");
    expect(formatBytes(Number.NaN)).toBe("—");
  });
});

describe("formatNumber", () => {
  it("adds group separators", () => {
    expect(formatNumber(1000)).toBe("1,000");
    expect(formatNumber(1234567)).toBe("1,234,567");
  });

  it("handles small and fractional values", () => {
    expect(formatNumber(0)).toBe("0");
    expect(formatNumber(12.5)).toBe("12.5");
  });

  it("returns an em-dash for null/NaN", () => {
    expect(formatNumber(null)).toBe("—");
    expect(formatNumber(Number.NaN)).toBe("—");
  });
});

describe("formatMs", () => {
  it("renders sub-millisecond as microseconds", () => {
    expect(formatMs(0.25)).toBe("250 µs");
  });

  it("renders milliseconds with scaled precision", () => {
    expect(formatMs(5)).toBe("5.00 ms");
    expect(formatMs(42.4)).toBe("42.4 ms");
    expect(formatMs(250)).toBe("250 ms");
  });

  it("renders seconds and minutes for longer durations", () => {
    expect(formatMs(1500)).toBe("1.50 s");
    expect(formatMs(90_000)).toBe("1m 30s");
  });

  it("returns an em-dash for null", () => {
    expect(formatMs(null)).toBe("—");
  });
});

describe("renderCell", () => {
  it("flags null as a styled NULL", () => {
    expect(renderCell(null)).toEqual({ text: "NULL", isNull: true });
  });

  it("renders primitives faithfully", () => {
    expect(renderCell("hi")).toEqual({ text: "hi", isNull: false });
    expect(renderCell(42)).toEqual({ text: "42", isNull: false });
    expect(renderCell(true)).toEqual({ text: "true", isNull: false });
  });
});
