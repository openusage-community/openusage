import { fireEvent, render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { OverviewPage } from "@/pages/overview"

describe("OverviewPage", () => {
  it("renders empty state", () => {
    render(<OverviewPage plugins={[]} displayMode="used" resetTimerDisplayMode="relative" />)
    expect(screen.getByText("No providers enabled")).toBeInTheDocument()
  })

  it("renders provider cards", () => {
    const plugins = [
      {
        meta: { id: "a", name: "Alpha", iconUrl: "icon", lines: [] },
        data: { providerId: "a", displayName: "Alpha", lines: [], iconUrl: "icon" },
        loading: false,
        error: null,
        lastManualRefreshAt: null,
        lastUpdatedAt: null,
      },
    ]
    render(<OverviewPage plugins={plugins} displayMode="used" resetTimerDisplayMode="relative" />)
    expect(screen.getByText("Total Spend")).toBeInTheDocument()
    expect(screen.getByText("Alpha")).toBeInTheDocument()
  })

  it("switches total spend periods", () => {
    const plugins = [
      {
        meta: { id: "a", name: "Alpha", iconUrl: "icon", brandColor: "#111111", lines: [] },
        data: {
          providerId: "a",
          displayName: "Alpha",
          lines: [
            { type: "text" as const, label: "Today", value: "$10.00 · 1K tokens" },
            { type: "text" as const, label: "Last 30 Days", value: "$30.00 · 3K tokens" },
          ],
          iconUrl: "icon",
        },
        loading: false,
        error: null,
        lastManualRefreshAt: null,
        lastUpdatedAt: null,
      },
    ]

    render(<OverviewPage plugins={plugins} displayMode="used" resetTimerDisplayMode="relative" />)
    expect(screen.getAllByText("$30.00").length).toBeGreaterThan(0)

    fireEvent.click(screen.getByRole("tab", { name: "Today" }))
    expect(screen.getAllByText("$10.00").length).toBeGreaterThan(0)
  })

  it("keeps total spend provider labels on one line with large amounts", () => {
    const plugins = [
      {
        meta: { id: "codex", name: "Codex", iconUrl: "icon", brandColor: "#111111", lines: [] },
        data: {
          providerId: "codex",
          displayName: "Codex",
          lines: [
            { type: "text" as const, label: "Last 30 Days", value: "$1,951.68 · 3M tokens" },
          ],
          iconUrl: "icon",
        },
        loading: false,
        error: null,
        lastManualRefreshAt: null,
        lastUpdatedAt: null,
      },
    ]

    render(<OverviewPage plugins={plugins} displayMode="used" resetTimerDisplayMode="relative" />)

    const providerLabel = screen
      .getAllByText("Codex")
      .find((element) => element.classList.contains("truncate"))

    expect(providerLabel).toBeDefined()
    expect(providerLabel).toHaveClass("truncate", "whitespace-nowrap")
    expect(screen.getByText("$1,951.68")).toHaveClass("shrink-0", "text-right")
  })

  it("shows total spend empty state without dollar data", () => {
    const plugins = [
      {
        meta: { id: "a", name: "Alpha", iconUrl: "icon", lines: [] },
        data: {
          providerId: "a",
          displayName: "Alpha",
          lines: [
            { type: "text" as const, label: "Last 30 Days", value: "3K tokens" },
          ],
          iconUrl: "icon",
        },
        loading: false,
        error: null,
        lastManualRefreshAt: null,
        lastUpdatedAt: null,
      },
    ]

    render(<OverviewPage plugins={plugins} displayMode="used" resetTimerDisplayMode="relative" />)
    expect(screen.getByText("No spend data for this period")).toBeInTheDocument()
  })

  it("only shows overview-scoped lines", () => {
    const plugins = [
      {
        meta: {
          id: "test",
          name: "Test",
          iconUrl: "icon",
          lines: [
            { type: "text" as const, label: "Primary", scope: "overview" as const },
            { type: "text" as const, label: "Secondary", scope: "detail" as const },
          ],
        },
        data: {
          providerId: "test",
          displayName: "Test",
          lines: [
            { type: "text" as const, label: "Primary", value: "Shown" },
            { type: "text" as const, label: "Secondary", value: "Hidden" },
          ],
          iconUrl: "icon",
        },
        loading: false,
        error: null,
        lastManualRefreshAt: null,
        lastUpdatedAt: null,
      },
    ]
    render(<OverviewPage plugins={plugins} displayMode="used" resetTimerDisplayMode="relative" />)
    expect(screen.getByText("Primary")).toBeInTheDocument()
    expect(screen.getByText("Shown")).toBeInTheDocument()
    expect(screen.queryByText("Secondary")).not.toBeInTheDocument()
    expect(screen.queryByText("Hidden")).not.toBeInTheDocument()
  })

  it("does not show provider quick links in combined view", () => {
    const plugins = [
      {
        meta: {
          id: "alpha",
          name: "Alpha",
          iconUrl: "icon",
          lines: [],
          links: [{ label: "Status", url: "https://status.example.com" }],
        },
        data: { providerId: "alpha", displayName: "Alpha", lines: [], iconUrl: "icon" },
        loading: false,
        error: null,
        lastManualRefreshAt: null,
        lastUpdatedAt: null,
      },
    ]

    render(<OverviewPage plugins={plugins} displayMode="used" resetTimerDisplayMode="relative" />)
    expect(screen.queryByRole("button", { name: /status/i })).toBeNull()
  })
})
