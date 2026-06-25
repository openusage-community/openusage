import { useCallback, useEffect, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { RefreshCw, X } from "lucide-react"
import { Button } from "@/components/ui/button"

type ErrorLogDay = {
  date: string
  count: number
}

type ErrorLogRead = {
  date: string
  content: string
  lineCount: number
}

type ErrorLogsDialogProps = {
  onClose: () => void
}

function countLabel(count: number): string {
  return `${count} ${count === 1 ? "error" : "errors"}`
}

export function ErrorLogsDialog({ onClose }: ErrorLogsDialogProps) {
  const [days, setDays] = useState<ErrorLogDay[]>([])
  const [selectedDate, setSelectedDate] = useState<string | null>(null)
  const [log, setLog] = useState<ErrorLogRead | null>(null)
  const [loadingDays, setLoadingDays] = useState(true)
  const [loadingLog, setLoadingLog] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [reloadKey, setReloadKey] = useState(0)

  const loadDays = useCallback(async () => {
    setLoadingDays(true)
    setError(null)
    try {
      const nextDays = await invoke<ErrorLogDay[]>("list_error_log_days")
      setDays(nextDays)
      const nextSelected = nextDays[0]?.date ?? null
      setSelectedDate((current) => current && nextDays.some((day) => day.date === current) ? current : nextSelected)
      if (nextDays.length === 0) {
        setLog(null)
      }
    } catch (loadError) {
      console.error("Failed to load error log days:", loadError)
      setError("Could not load error logs.")
    } finally {
      setLoadingDays(false)
    }
  }, [])

  const refreshLogs = useCallback(async () => {
    await loadDays()
    setReloadKey((current) => current + 1)
  }, [loadDays])

  useEffect(() => {
    void loadDays()
  }, [loadDays])

  useEffect(() => {
    if (!selectedDate) return
    let cancelled = false
    setLoadingLog(true)
    setError(null)
    invoke<ErrorLogRead>("read_error_log_day", { date: selectedDate })
      .then((nextLog) => {
        if (!cancelled) setLog(nextLog)
      })
      .catch((loadError) => {
        if (cancelled) return
        console.error("Failed to read error log day:", loadError)
        setError("Could not read this log.")
      })
      .finally(() => {
        if (!cancelled) setLoadingLog(false)
      })
    return () => {
      cancelled = true
    }
  }, [reloadKey, selectedDate])

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault()
        onClose()
      }
    }
    document.addEventListener("keydown", handleKeyDown)
    return () => document.removeEventListener("keydown", handleKeyDown)
  }, [onClose])

  const handleBackdropClick = (event: React.MouseEvent) => {
    if (event.target === event.currentTarget) {
      onClose()
    }
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm rounded-xl"
      onClick={handleBackdropClick}
    >
      <div className="bg-card rounded-lg border shadow-xl p-4 max-w-[31rem] w-[calc(100%-2rem)] max-h-[calc(100%-2rem)] mx-4 animate-in fade-in zoom-in-95 duration-200 flex flex-col">
        <div className="flex items-start justify-between gap-3 mb-3">
          <div>
            <h2 className="text-sm font-semibold">Error Logs</h2>
            <p className="text-xs text-muted-foreground">Local app errors by day</p>
          </div>
          <div className="flex items-center gap-1">
            <Button
              variant="ghost"
              size="icon-xs"
              aria-label="Refresh logs"
              onClick={() => void refreshLogs()}
            >
              <RefreshCw className="size-3" />
            </Button>
            <Button variant="ghost" size="icon-xs" aria-label="Close error logs" onClick={onClose}>
              <X className="size-3" />
            </Button>
          </div>
        </div>

        {loadingDays ? (
          <div className="text-xs text-muted-foreground py-4">Loading logs...</div>
        ) : days.length === 0 ? (
          <div className="text-xs text-muted-foreground py-4">No error logs yet.</div>
        ) : (
          <div className="grid grid-cols-[8.5rem_minmax(0,1fr)] gap-3 min-h-0">
            <div className="space-y-1 overflow-y-auto max-h-72 pr-1">
              {days.map((day) => (
                <button
                  key={day.date}
                  type="button"
                  aria-label={`${day.date}, ${countLabel(day.count)}`}
                  aria-pressed={day.date === selectedDate}
                  onClick={() => setSelectedDate(day.date)}
                  className={[
                    "w-full rounded-md px-2 py-1.5 text-left text-xs transition-colors",
                    day.date === selectedDate
                      ? "bg-primary text-primary-foreground"
                      : "bg-muted/50 text-foreground hover:bg-muted",
                  ].join(" ")}
                >
                  <span className="block font-medium tabular-nums">{day.date}</span>
                  <span className={day.date === selectedDate ? "text-primary-foreground/80" : "text-muted-foreground"}>
                    {countLabel(day.count)}
                  </span>
                </button>
              ))}
            </div>
            <div className="min-w-0">
              {error ? (
                <div className="text-xs text-destructive rounded-md border border-destructive/40 p-2">
                  {error}
                </div>
              ) : loadingLog ? (
                <div className="text-xs text-muted-foreground py-4">Loading log...</div>
              ) : (
                <pre className="max-h-72 overflow-auto rounded-md border bg-background p-2 text-[11px] leading-4 text-foreground whitespace-pre-wrap select-text">
                  {log?.content || "No errors for this day."}
                </pre>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  )
}
