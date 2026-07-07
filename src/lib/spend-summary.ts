import type { MetricLine, PluginDisplayState } from "@/lib/plugin-types"

export type SpendPeriod = "today" | "yesterday" | "30d"

export type SpendSummaryRow = {
  id: string
  name: string
  amount: number
  color: string
}

export type SpendSummary = {
  period: SpendPeriod
  total: number
  rows: SpendSummaryRow[]
}

const FALLBACK_COLORS = [
  "var(--chart-1)",
  "var(--chart-2)",
  "var(--chart-3)",
  "var(--chart-4)",
  "var(--chart-5)",
]

const PERIOD_LABELS: Record<Exclude<SpendPeriod, "30d">, string> = {
  today: "Today",
  yesterday: "Yesterday",
}

const MONTHLY_FALLBACK_LABELS = new Set([
  "Total usage",
  "Monthly",
  "Extra usage spent",
])

export function parseDollarAmount(value: string): number | null {
  const match = value.match(/\$\s*([0-9][0-9,]*(?:\.\d+)?)/)
  if (!match) return null

  const amount = Number(match[1].replace(/,/g, ""))
  return Number.isFinite(amount) ? amount : null
}

function getLineDollarAmount(line: MetricLine): number | null {
  if (line.type === "progress" && line.format.kind === "dollars") {
    return Number.isFinite(line.used) ? line.used : null
  }

  if (line.type === "text") {
    return parseDollarAmount(line.value)
  }

  return null
}

function findSpendAmount(lines: MetricLine[], period: SpendPeriod): number | null {
  if (period !== "30d") {
    const label = PERIOD_LABELS[period]
    const line = lines.find((candidate) => candidate.label === label)
    return line ? getLineDollarAmount(line) : null
  }

  const exactLine = lines.find((candidate) => candidate.label === "Last 30 Days")
  const exactAmount = exactLine ? getLineDollarAmount(exactLine) : null
  if (exactAmount !== null) return exactAmount

  const fallbackLine = lines.find((candidate) => MONTHLY_FALLBACK_LABELS.has(candidate.label))
  return fallbackLine ? getLineDollarAmount(fallbackLine) : null
}

export function buildSpendSummary(
  plugins: PluginDisplayState[],
  period: SpendPeriod
): SpendSummary {
  const rows = plugins.flatMap((plugin, index) => {
    const amount = findSpendAmount(plugin.data?.lines ?? [], period)
    if (amount === null || amount <= 0) return []

    return [{
      id: plugin.meta.id,
      name: plugin.meta.name,
      amount,
      color: plugin.meta.brandColor || FALLBACK_COLORS[index % FALLBACK_COLORS.length],
    }]
  })

  rows.sort((a, b) => b.amount - a.amount)

  return {
    period,
    total: rows.reduce((sum, row) => sum + row.amount, 0),
    rows,
  }
}

export function formatSpendAmount(value: number): string {
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  }).format(Number.isFinite(value) ? value : 0)
}

export function formatCompactSpendAmount(value: number): string {
  const safeValue = Number.isFinite(value) ? value : 0
  if (Math.abs(safeValue) < 1000) return formatSpendAmount(safeValue)

  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    notation: "compact",
    maximumFractionDigits: 1,
  }).format(safeValue)
}
