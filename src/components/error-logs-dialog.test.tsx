import { render, screen, waitFor } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { beforeEach, describe, expect, it, vi } from "vitest"

const invokeMock = vi.hoisted(() => vi.fn())

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}))

import { ErrorLogsDialog } from "@/components/error-logs-dialog"

function textIncludes(value: string) {
  return (_content: string, element: Element | null) =>
    element?.tagName === "PRE" && (element.textContent?.includes(value) ?? false)
}

describe("ErrorLogsDialog", () => {
  let todayContent: string

  beforeEach(() => {
    todayContent = "today error\nsecond error"
    invokeMock.mockReset()
    invokeMock.mockImplementation(async (command: string, args?: unknown) => {
      if (command === "list_error_log_days") {
        return [
          { date: "2026-06-25", count: 2 },
          { date: "2026-06-24", count: 1 },
        ]
      }
      if (command === "read_error_log_day") {
        const request = args as { date: string }
        return {
          date: request.date,
          content: request.date === "2026-06-25" ? todayContent : "older error",
          lineCount: request.date === "2026-06-25" ? 2 : 1,
        }
      }
      return null
    })
  })

  it("loads newest error log day on open", async () => {
    render(<ErrorLogsDialog onClose={vi.fn()} />)

    expect(await screen.findByText("Error Logs")).toBeInTheDocument()
    expect(await screen.findByText(textIncludes("today error"))).toBeInTheDocument()
    expect(screen.getByRole("button", { name: "2026-06-25, 2 errors" })).toHaveAttribute("aria-pressed", "true")
  })

  it("switches between available days", async () => {
    render(<ErrorLogsDialog onClose={vi.fn()} />)

    await screen.findByText(textIncludes("today error"))
    await userEvent.click(screen.getByRole("button", { name: "2026-06-24, 1 error" }))

    expect(await screen.findByText("older error")).toBeInTheDocument()
    expect(invokeMock).toHaveBeenLastCalledWith("read_error_log_day", { date: "2026-06-24" })
  })

  it("shows empty state when no error logs exist", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "list_error_log_days") return []
      return null
    })

    render(<ErrorLogsDialog onClose={vi.fn()} />)

    expect(await screen.findByText("No error logs yet.")).toBeInTheDocument()
  })

  it("refreshes the day list and closes with Escape", async () => {
    const onClose = vi.fn()
    render(<ErrorLogsDialog onClose={onClose} />)

    await screen.findByText(textIncludes("today error"))
    await userEvent.click(screen.getByRole("button", { name: "Refresh logs" }))
    expect(invokeMock).toHaveBeenCalledWith("list_error_log_days")

    await userEvent.keyboard("{Escape}")
    await waitFor(() => expect(onClose).toHaveBeenCalled())
  })

  it("reloads the selected day when refreshed", async () => {
    render(<ErrorLogsDialog onClose={vi.fn()} />)

    await screen.findByText(textIncludes("today error"))
    todayContent = "new error after refresh"
    await userEvent.click(screen.getByRole("button", { name: "Refresh logs" }))

    expect(await screen.findByText(textIncludes("new error after refresh"))).toBeInTheDocument()
  })
})
