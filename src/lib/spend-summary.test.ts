import { describe, expect, it } from "vitest"
import { buildSpendSummary, parseDollarAmount } from "@/lib/spend-summary"
import type { MetricLine, PluginDisplayState } from "@/lib/plugin-types"

function plugin(
  id: string,
  name: string,
  lines: MetricLine[],
  brandColor?: string
): PluginDisplayState {
  return {
    meta: {
      id,
      name,
      iconUrl: "",
      brandColor,
      lines: [],
      primaryCandidates: [],
    },
    data: {
      providerId: id,
      displayName: name,
      lines,
      iconUrl: "",
    },
    loading: false,
    error: null,
    lastManualRefreshAt: null,
    lastUpdatedAt: null,
  }
}

describe("spend summary", () => {
  it("parses the first dollar amount from text", () => {
    expect(parseDollarAmount("$13.40 · 1.2K tokens")).toBe(13.4)
    expect(parseDollarAmount("tokens only")).toBeNull()
  })

  it("sums Today spend across providers", () => {
    const summary = buildSpendSummary([
      plugin("claude", "Claude", [
        { type: "text", label: "Today", value: "$6.04 · 12K tokens" },
      ]),
      plugin("codex", "Codex", [
        { type: "progress", label: "Today", used: 3.5, limit: 10, format: { kind: "dollars" } },
      ]),
    ], "today")

    expect(summary.total).toBe(9.54)
    expect(summary.rows.map((row) => row.name)).toEqual(["Claude", "Codex"])
  })

  it("hides providers without dollar spend", () => {
    const summary = buildSpendSummary([
      plugin("claude", "Claude", [
        { type: "text", label: "Today", value: "12K tokens" },
      ]),
      plugin("cursor", "Cursor", [
        { type: "progress", label: "Total usage", used: 50, limit: 100, format: { kind: "percent" } },
      ]),
    ], "today")

    expect(summary.total).toBe(0)
    expect(summary.rows).toEqual([])
  })

  it("uses Last 30 Days before monthly fallback", () => {
    const summary = buildSpendSummary([
      plugin("claude", "Claude", [
        { type: "text", label: "Last 30 Days", value: "$10.00 · 20K tokens" },
        { type: "progress", label: "Total usage", used: 100, limit: 200, format: { kind: "dollars" } },
      ]),
      plugin("cursor", "Cursor", [
        { type: "progress", label: "Total usage", used: 20, limit: 100, format: { kind: "dollars" } },
      ]),
    ], "30d")

    expect(summary.total).toBe(30)
    expect(summary.rows.find((row) => row.id === "claude")?.amount).toBe(10)
    expect(summary.rows.find((row) => row.id === "cursor")?.amount).toBe(20)
  })
})
