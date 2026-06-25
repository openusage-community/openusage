import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

const { invokeMock, logErrorMock, logWarnMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  logErrorMock: vi.fn(),
  logWarnMock: vi.fn(),
}))

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}))

vi.mock("@tauri-apps/plugin-log", () => ({
  error: logErrorMock,
  warn: logWarnMock,
}))

import { installFrontendErrorLogging, resetFrontendErrorLoggingForTests } from "@/lib/frontend-error-logging"

describe("installFrontendErrorLogging", () => {
  let originalError: typeof console.error
  let originalWarn: typeof console.warn

  beforeEach(() => {
    originalError = console.error
    originalWarn = console.warn
    invokeMock.mockReset()
    logErrorMock.mockReset()
    logWarnMock.mockReset()
    invokeMock.mockResolvedValue(undefined)
    logErrorMock.mockResolvedValue(undefined)
    logWarnMock.mockResolvedValue(undefined)
  })

  afterEach(() => {
    resetFrontendErrorLoggingForTests()
    console.error = originalError
    console.warn = originalWarn
  })

  it("records console.error through Tauri IPC and keeps existing log forwarding", async () => {
    const originalErrorSpy = vi.fn()
    console.error = originalErrorSpy

    installFrontendErrorLogging()
    console.error("boom", new Error("broken"))
    await Promise.resolve()

    expect(originalErrorSpy).toHaveBeenCalledWith("boom", expect.any(Error))
    expect(logErrorMock).toHaveBeenCalledWith("boom Error: broken")
    expect(invokeMock).toHaveBeenCalledWith("record_frontend_error", {
      source: "console.error",
      message: "boom Error: broken",
      stack: expect.stringContaining("Error: broken"),
    })
  })

  it("records window error and unhandled rejection events", async () => {
    installFrontendErrorLogging()

    window.dispatchEvent(new ErrorEvent("error", {
      message: "window crashed",
      error: new Error("window crashed"),
    }))
    const rejection = new Event("unhandledrejection")
    Object.defineProperty(rejection, "reason", { value: new Error("promise failed") })
    window.dispatchEvent(rejection)
    await Promise.resolve()

    expect(invokeMock).toHaveBeenCalledWith("record_frontend_error", {
      source: "window.error",
      message: "window crashed",
      stack: expect.stringContaining("Error: window crashed"),
    })
    expect(invokeMock).toHaveBeenCalledWith("record_frontend_error", {
      source: "unhandledrejection",
      message: "Error: promise failed",
      stack: expect.stringContaining("Error: promise failed"),
    })
  })

  it("does not recurse when error recording fails", async () => {
    const originalErrorSpy = vi.fn()
    console.error = originalErrorSpy
    invokeMock.mockRejectedValue(new Error("ipc failed"))

    installFrontendErrorLogging()
    console.error("boom")
    await Promise.resolve()

    expect(originalErrorSpy).toHaveBeenCalledTimes(1)
    expect(invokeMock).toHaveBeenCalledTimes(1)
  })
})
