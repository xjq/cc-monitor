export function formatTokens(tokens: number): string {
  if (tokens === 0) return "0.0";

  const abs = Math.abs(tokens);
  let value: number;
  let suffix: string;

  if (abs >= 1_000_000) {
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

function formatCurrency(amount: number, symbol: string): string {
  const sign = amount < 0 ? "-" : "";
  const absValue = Math.abs(amount);
  return `${sign}${symbol}${absValue.toFixed(2)}`;
}

export function formatUsd(n: number): string {
  return formatCurrency(n, "$");
}

export function formatCny(usd: number, rate: number): string {
  return formatCurrency(usd * rate, "¥");
}
