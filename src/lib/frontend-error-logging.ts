import { invoke } from "@tauri-apps/api/core"
import { error as logError, warn as logWarn } from "@tauri-apps/plugin-log"

let restoreCurrent: (() => void) | null = null

function stringify(arg: unknown): string {
  if (arg === null) return "null"
  if (arg === undefined) return "undefined"
  if (typeof arg === "string") return arg
  if (arg instanceof Error) return `${arg.name}: ${arg.message}`
  try {
    return JSON.stringify(arg)
  } catch {
    return String(arg)
  }
}

function stackFromUnknown(value: unknown): string | null {
  if (value instanceof Error) return value.stack || `${value.name}: ${value.message}`
  return null
}

function messageFromUnknown(value: unknown): string {
  return stringify(value)
}

function recordFrontendError(source: string, message: string, stack: string | null) {
  void invoke("record_frontend_error", { source, message, stack }).catch(() => {})
}

export function installFrontendErrorLogging(): () => void {
  if (restoreCurrent) return restoreCurrent

  const originalError = console.error
  const originalWarn = console.warn

  console.error = (...args: unknown[]) => {
    originalError(...args)
    const message = args.map(stringify).join(" ")
    logError(message).catch(() => {})
    recordFrontendError("console.error", message, args.map(stackFromUnknown).find(Boolean) ?? null)
  }

  console.warn = (...args: unknown[]) => {
    originalWarn(...args)
    logWarn(args.map(stringify).join(" ")).catch(() => {})
  }

  const handleError = (event: ErrorEvent) => {
    recordFrontendError(
      "window.error",
      event.message || messageFromUnknown(event.error),
      stackFromUnknown(event.error)
    )
  }

  const handleUnhandledRejection = (event: PromiseRejectionEvent) => {
    recordFrontendError(
      "unhandledrejection",
      messageFromUnknown(event.reason),
      stackFromUnknown(event.reason)
    )
  }

  window.addEventListener("error", handleError)
  window.addEventListener("unhandledrejection", handleUnhandledRejection)

  restoreCurrent = () => {
    console.error = originalError
    console.warn = originalWarn
    window.removeEventListener("error", handleError)
    window.removeEventListener("unhandledrejection", handleUnhandledRejection)
    restoreCurrent = null
  }

  return restoreCurrent
}

export function resetFrontendErrorLoggingForTests() {
  restoreCurrent?.()
}
