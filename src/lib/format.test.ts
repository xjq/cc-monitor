import { describe, it, expect } from "vitest";
import { formatTokens, formatHours, formatUsd, formatCny } from "./format";

describe("formatTokens", () => {
  it("formats zero", () => {
    expect(formatTokens(0)).toBe("0.0");
  });

  it("formats small numbers", () => {
    expect(formatTokens(123)).toBe("123.0");
    expect(formatTokens(999)).toBe("999.0");
  });

  it("formats thousands as K", () => {
    expect(formatTokens(1000)).toBe("1.0K");
    expect(formatTokens(1500)).toBe("1.5K");
    expect(formatTokens(9999)).toBe("10.0K");
    expect(formatTokens(999999)).toBe("1000.0K");
  });

  it("formats millions as M", () => {
    expect(formatTokens(1_000_000)).toBe("1.0M");
    expect(formatTokens(1_500_000)).toBe("1.5M");
    expect(formatTokens(9_999_999)).toBe("10.0M");
    expect(formatTokens(999_999_999)).toBe("1000.0M");
  });

  it("rounds to one decimal place", () => {
    expect(formatTokens(1234)).toBe("1.2K");
    expect(formatTokens(12345)).toBe("12.3K");
    expect(formatTokens(123456)).toBe("123.5K");
  });
});

describe("formatHours", () => {
  it("formats zero hours", () => {
    expect(formatHours(0)).toBe("0.0h");
  });

  it("formats fractional hours", () => {
    expect(formatHours(0.5)).toBe("0.5h");
    expect(formatHours(1.25)).toBe("1.3h");
    expect(formatHours(23.99)).toBe("24.0h");
  });

  it("formats large hour values", () => {
    expect(formatHours(100)).toBe("100.0h");
    expect(formatHours(999.9)).toBe("999.9h");
  });
});

describe("formatUsd", () => {
  it("formats USD with $ symbol", () => {
    expect(formatUsd(0)).toBe("$0.00");
    expect(formatUsd(100)).toBe("$100.00");
    expect(formatUsd(1234.567)).toBe("$1234.57");
  });

  it("handles negative values", () => {
    expect(formatUsd(-100)).toBe("-$100.00");
    expect(formatUsd(-50)).toBe("-$50.00");
  });
});

describe("formatCny", () => {
  it("formats CNY with ¥ symbol and converts from USD", () => {
    expect(formatCny(0, 7.2)).toBe("¥0.00");
    expect(formatCny(100, 7.2)).toBe("¥720.00");
    expect(formatCny(50, 7.0)).toBe("¥350.00");
    expect(formatCny(1234.567, 7.2)).toBe("¥8888.88");
  });

  it("handles negative values", () => {
    expect(formatCny(-100, 7.2)).toBe("-¥720.00");
  });
});
