import { useMemo, useState } from "react"
import type { PluginDisplayState } from "@/lib/plugin-types"
import {
  buildSpendSummary,
  formatCompactSpendAmount,
  formatSpendAmount,
  type SpendPeriod,
} from "@/lib/spend-summary"
import { cn } from "@/lib/utils"

type TotalSpendChartProps = {
  plugins: PluginDisplayState[]
}

const PERIOD_OPTIONS: Array<{ value: SpendPeriod; label: string }> = [
  { value: "today", label: "Today" },
  { value: "yesterday", label: "Yesterday" },
  { value: "30d", label: "30 Days" },
]

const DONUT_RADIUS = 44
const DONUT_STROKE = 18
const DONUT_CIRCUMFERENCE = 2 * Math.PI * DONUT_RADIUS
const DONUT_GAP = 4

export function TotalSpendChart({ plugins }: TotalSpendChartProps) {
  const [period, setPeriod] = useState<SpendPeriod>("30d")
  const summary = useMemo(() => buildSpendSummary(plugins, period), [plugins, period])
  const hasSpend = summary.total > 0 && summary.rows.length > 0

  return (
    <section className="pb-3">
      <div className="mb-2 flex items-center justify-between">
        <h2 className="text-lg font-semibold">Total Spend</h2>
      </div>

      <div className="rounded-lg bg-muted/55 p-3">
        <div
          className="mb-4 grid h-9 grid-cols-3 rounded-full bg-background/55 p-1"
          role="tablist"
          aria-label="Spend period"
        >
          {PERIOD_OPTIONS.map((option) => (
            <button
              key={option.value}
              type="button"
              role="tab"
              aria-selected={period === option.value}
              className={cn(
                "rounded-full px-2 text-sm font-medium text-muted-foreground transition",
                "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
                period === option.value
                  ? "bg-background text-foreground shadow-sm"
                  : "hover:text-foreground"
              )}
              onClick={() => setPeriod(option.value)}
            >
              {option.label}
            </button>
          ))}
        </div>

        {hasSpend ? (
          <div className="grid grid-cols-[118px_minmax(0,1fr)] items-center gap-3">
            <Donut rows={summary.rows} total={summary.total} />
            <div className="min-w-0 space-y-2">
              {summary.rows.map((row) => (
                <div key={row.id} className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-3">
                  <div className="flex min-w-0 items-start gap-2">
                    <span
                      className="mt-1 h-2.5 w-2.5 flex-shrink-0 rounded-full"
                      style={{ backgroundColor: row.color }}
                    />
                    <span
                      className={cn(
                        "min-w-0 break-words leading-tight text-foreground",
                        row.name.length > 22
                          ? "text-[11px]"
                          : row.name.length > 14
                            ? "text-xs"
                            : "text-sm"
                      )}
                    >
                      {row.name}
                    </span>
                  </div>
                  <span className="text-sm tabular-nums text-muted-foreground">
                    {formatSpendAmount(row.amount)}
                  </span>
                </div>
              ))}
            </div>
          </div>
        ) : (
          <div className="flex h-[126px] items-center justify-center text-center text-sm text-muted-foreground">
            No spend data for this period
          </div>
        )}
      </div>
    </section>
  )
}

function Donut({
  rows,
  total,
}: {
  rows: Array<{ id: string; amount: number; color: string }>
  total: number
}) {
  let offset = 0

  return (
    <div className="relative h-[118px] w-[118px]">
      <svg
        className="h-full w-full -rotate-90"
        viewBox="0 0 126 126"
        role="img"
        aria-label={`Total spend ${formatSpendAmount(total)}`}
      >
        <circle
          cx="63"
          cy="63"
          r={DONUT_RADIUS}
          fill="none"
          stroke="var(--background)"
          strokeWidth={DONUT_STROKE}
        />
        {rows.map((row) => {
          const length = Math.max(0, (row.amount / total) * DONUT_CIRCUMFERENCE - DONUT_GAP)
          const dashOffset = -offset
          offset += (row.amount / total) * DONUT_CIRCUMFERENCE

          return (
            <circle
              key={row.id}
              cx="63"
              cy="63"
              r={DONUT_RADIUS}
              fill="none"
              stroke={row.color}
              strokeWidth={DONUT_STROKE}
              strokeLinecap="round"
              strokeDasharray={`${length} ${DONUT_CIRCUMFERENCE}`}
              strokeDashoffset={dashOffset}
            />
          )
        })}
      </svg>
      <div className="absolute inset-0 flex items-center justify-center">
        <span className="text-lg font-semibold tabular-nums">
          {formatCompactSpendAmount(total)}
        </span>
      </div>
    </div>
  )
}
