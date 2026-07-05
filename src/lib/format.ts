export function formatTokens(tokens: number): string {
  if (tokens === 0) return "0.0";

  const abs = Math.abs(tokens);
  let value: number;
  let suffix: string;

  if (abs >= 1_000_000_000) {
    value = tokens / 1_000_000_000;
    suffix = "B";
  } else if (abs >= 1_000_000) {
    value = tokens / 1_000_000;
    suffix = "M";
  } else if (abs >= 1_000) {
    value = tokens / 1_000;
    suffix = "K";
  } else {
    return `${tokens.toFixed(1)}`;
  }

  return `${value.toFixed(1)}${suffix}`;
}

export function formatHours(hours: number): string {
  return `${hours.toFixed(1)}h`;
}

export function formatCurrency(amount: number, currency: "USD" | "CNY", usdToCny?: number): string {
  const symbol = currency === "USD" ? "$" : "¥";
  let value = amount;

  if (currency === "CNY" && usdToCny !== undefined) {
    value = amount * usdToCny;
  }

  const sign = value < 0 ? "-" : "";
  const absValue = Math.abs(value);

  return `${sign}${symbol}${absValue.toFixed(2)}`;
}
