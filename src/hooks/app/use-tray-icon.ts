import { useCallback, useEffect, useRef, useState } from "react"
import { resolveResource } from "@tauri-apps/api/path"
import { TrayIcon } from "@tauri-apps/api/tray"
import { isTauri } from "@tauri-apps/api/core"
import type { PluginMeta } from "@/lib/plugin-types"
import type { DisplayMode, MenubarIconStyle, PluginSettings, ThemeMode } from "@/lib/settings"
import { getEnabledPluginIds } from "@/lib/settings"
import { getTrayIconSizePx, renderTrayBarsIcon } from "@/lib/tray-bars-icon"
import { getTrayPrimaryBars, type TrayPrimaryBar } from "@/lib/tray-primary-progress"
import { formatTrayPercentText, formatTrayTooltip } from "@/lib/tray-tooltip"
import type { PluginState } from "@/hooks/app/types"

type TrayUpdateReason = "probe" | "settings" | "init"

function getSystemDarkMode(): boolean {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return false
  }

  return window.matchMedia("(prefers-color-scheme: dark)").matches
}

function isLinuxTrayRuntime(): boolean {
  return Boolean(
    isTauri() &&
    typeof navigator !== "undefined" &&
    /linux/i.test(navigator.userAgent)
  )
}

function getTrayForegroundColor(themeMode: ThemeMode, isLinuxTray: boolean): "black" | "white" {
  if (isLinuxTray) {
    return getSystemDarkMode() ? "white" : "black"
  }

  if (themeMode === "dark") return "white"
  if (themeMode === "light") return "black"

  if (typeof document !== "undefined") {
    if (document.documentElement.classList.contains("dark")) return "white"
    if (document.documentElement.classList.contains("light")) return "black"
  }

  return getSystemDarkMode() ? "white" : "black"
}

type UseTrayIconArgs = {
  pluginsMeta: PluginMeta[]
  pluginSettings: PluginSettings | null
  pluginStates: Record<string, PluginState>
  displayMode: DisplayMode
  menubarIconStyle: MenubarIconStyle
  activeView: string
  themeMode: ThemeMode
}

export type TraySettingsPreview = {
  bars: TrayPrimaryBar[]
  providerBars: TrayPrimaryBar[]
  providerIconUrl?: string
  providerPercentText: string
}

const EMPTY_TRAY_SETTINGS_PREVIEW: TraySettingsPreview = {
  bars: [],
  providerBars: [],
  providerPercentText: "--%",
}

function isSameTraySettingsPreview(a: TraySettingsPreview, b: TraySettingsPreview): boolean {
  if (a.providerIconUrl !== b.providerIconUrl) return false
  if (a.providerPercentText !== b.providerPercentText) return false
  if (a.bars.length !== b.bars.length) return false
  if (a.providerBars.length !== b.providerBars.length) return false
  for (let i = 0; i < a.bars.length; i += 1) {
    if (a.bars[i]?.id !== b.bars[i]?.id) return false
    if (a.bars[i]?.fraction !== b.bars[i]?.fraction) return false
  }
  for (let i = 0; i < a.providerBars.length; i += 1) {
    if (a.providerBars[i]?.id !== b.providerBars[i]?.id) return false
    if (a.providerBars[i]?.fraction !== b.providerBars[i]?.fraction) return false
  }
  return true
}

export function useTrayIcon({
  pluginsMeta,
  pluginSettings,
  pluginStates,
  displayMode,
  menubarIconStyle,
  activeView,
  themeMode,
}: UseTrayIconArgs) {
  const trayRef = useRef<TrayIcon | null>(null)
  const trayGaugeIconPathRef = useRef<string | null>(null)
  const trayUpdateTimerRef = useRef<number | null>(null)
  const trayUpdatePendingRef = useRef(false)
  const trayUpdateQueuedRef = useRef(false)
  const trayUpdateGenerationRef = useRef(0)
  const [trayReady, setTrayReady] = useState(false)
  const [traySettingsPreview, setTraySettingsPreview] = useState<TraySettingsPreview>(
    EMPTY_TRAY_SETTINGS_PREVIEW
  )

  const pluginsMetaRef = useRef(pluginsMeta)
  const pluginSettingsRef = useRef(pluginSettings)
  const pluginStatesRef = useRef(pluginStates)
  const displayModeRef = useRef(displayMode)
  const menubarIconStyleRef = useRef(menubarIconStyle)
  const activeViewRef = useRef(activeView)
  const themeModeRef = useRef(themeMode)
  const lastTrayProviderIdRef = useRef<string | null>(null)
  const initialIsLinuxTray = isLinuxTrayRuntime()
  const isLinuxTrayRef = useRef(initialIsLinuxTray)
  const trayForegroundColorRef = useRef<"black" | "white">(
    getTrayForegroundColor(themeMode, initialIsLinuxTray)
  )

  useEffect(() => {
    pluginsMetaRef.current = pluginsMeta
  }, [pluginsMeta])

  useEffect(() => {
    pluginSettingsRef.current = pluginSettings
  }, [pluginSettings])

  useEffect(() => {
    pluginStatesRef.current = pluginStates
  }, [pluginStates])

  useEffect(() => {
    displayModeRef.current = displayMode
  }, [displayMode])

  useEffect(() => {
    menubarIconStyleRef.current = menubarIconStyle
  }, [menubarIconStyle])

  useEffect(() => {
    activeViewRef.current = activeView
  }, [activeView])

  const scheduleTrayIconUpdate = useCallback((
    _reason: TrayUpdateReason,
    delayMs = 0,
  ) => {
    const updateGeneration = trayUpdateGenerationRef.current + 1
    trayUpdateGenerationRef.current = updateGeneration

    if (trayUpdateTimerRef.current !== null) {
      window.clearTimeout(trayUpdateTimerRef.current)
      trayUpdateTimerRef.current = null
    }

    trayUpdateTimerRef.current = window.setTimeout(() => {
      trayUpdateTimerRef.current = null
      if (trayUpdatePendingRef.current) {
        trayUpdateQueuedRef.current = true
        return
      }
      trayUpdatePendingRef.current = true
      const isCurrentUpdate = () => trayUpdateGenerationRef.current === updateGeneration

      const finalizeUpdate = () => {
        trayUpdatePendingRef.current = false
        if (!trayUpdateQueuedRef.current) return
        trayUpdateQueuedRef.current = false
        scheduleTrayIconUpdate("probe", 0)
      }

      const tray = trayRef.current
      if (!tray) {
        finalizeUpdate()
        return
      }

      const maybeSetTitle = (tray as TrayIcon & { setTitle?: (value: string) => Promise<void> }).setTitle
      const setTitleFn =
        typeof maybeSetTitle === "function" ? (value: string) => maybeSetTitle.call(tray, value) : null
      const supportsNativeTrayTitle = setTitleFn !== null && !isLinuxTrayRef.current
      const setTrayTitle = (title: string) => {
        if (isLinuxTrayRef.current || !setTitleFn) {
          return Promise.resolve()
        }
        return setTitleFn(title)
      }

      const maybeSetIconAsTemplate =
        (tray as TrayIcon & { setIconAsTemplate?: (asTemplate: boolean) => Promise<void> }).setIconAsTemplate
      const setIconAsTemplateFn =
        typeof maybeSetIconAsTemplate === "function"
          ? (value: boolean) => maybeSetIconAsTemplate.call(tray, value)
          : null
      const setTrayIconTemplate = (value: boolean) => {
        if (isLinuxTrayRef.current || !setIconAsTemplateFn) {
          return Promise.resolve()
        }
        return setIconAsTemplateFn(value)
      }

      const maybeSetTooltip = (tray as TrayIcon & { setTooltip?: (value: string) => Promise<void> }).setTooltip
      const setTooltipFn =
        typeof maybeSetTooltip === "function" ? (value: string) => maybeSetTooltip.call(tray, value) : null
      const setTrayTooltip = (tooltip: string) => {
        if (isLinuxTrayRef.current || !setTooltipFn) {
          return Promise.resolve()
        }
        return setTooltipFn(tooltip)
      }

      const restoreGaugeIcon = () => {
        const gaugePath = trayGaugeIconPathRef.current
        if (gaugePath) {
          Promise.all([
            tray.setIcon(gaugePath),
            setTrayIconTemplate(true),
            setTrayTitle(""),
            setTrayTooltip("OpenUsage"),
          ])
            .catch((e) => {
              console.error("Failed to restore tray gauge icon:", e)
            })
            .finally(() => {
              finalizeUpdate()
            })
        } else {
          finalizeUpdate()
        }
      }

      const currentSettings = pluginSettingsRef.current
      if (!currentSettings) {
        setTraySettingsPreview(EMPTY_TRAY_SETTINGS_PREVIEW)
        restoreGaugeIcon()
        return
      }

      const enabledPluginIds = getEnabledPluginIds(currentSettings)
      if (enabledPluginIds.length === 0) {
        setTraySettingsPreview(EMPTY_TRAY_SETTINGS_PREVIEW)
        restoreGaugeIcon()
        return
      }

      const style = menubarIconStyleRef.current
      const sizePx = getTrayIconSizePx(window.devicePixelRatio)
      const foregroundColor = trayForegroundColorRef.current
      const nextActiveView = activeViewRef.current
      const activeProviderId =
        nextActiveView !== "home" && nextActiveView !== "settings" ? nextActiveView : null

      let trayProviderId: string | null = null
      if (activeProviderId && enabledPluginIds.includes(activeProviderId)) {
        trayProviderId = activeProviderId
      } else if (
        lastTrayProviderIdRef.current &&
        enabledPluginIds.includes(lastTrayProviderIdRef.current)
      ) {
        trayProviderId = lastTrayProviderIdRef.current
      } else {
        trayProviderId = enabledPluginIds[0] ?? null
      }

      const barsForPreview = getTrayPrimaryBars({
        pluginsMeta: pluginsMetaRef.current,
        pluginSettings: currentSettings,
        pluginStates: pluginStatesRef.current,
        maxBars: 4,
        displayMode: displayModeRef.current,
      })

      const providerBars = trayProviderId
        ? getTrayPrimaryBars({
            pluginsMeta: pluginsMetaRef.current,
            pluginSettings: currentSettings,
            pluginStates: pluginStatesRef.current,
            maxBars: 1,
            displayMode: displayModeRef.current,
            pluginId: trayProviderId,
          })
        : []

      const providerIconUrl = trayProviderId
        ? pluginsMetaRef.current.find((plugin) => plugin.id === trayProviderId)?.iconUrl
        : undefined
      const providerPercentText = formatTrayPercentText(providerBars[0]?.fraction)

      const nextPreview: TraySettingsPreview = {
        bars: barsForPreview,
        providerBars,
        providerIconUrl,
        providerPercentText,
      }
      setTraySettingsPreview((prev) =>
        isSameTraySettingsPreview(prev, nextPreview) ? prev : nextPreview
      )

      const tooltipBars = getTrayPrimaryBars({
        pluginsMeta: pluginsMetaRef.current,
        pluginSettings: currentSettings,
        pluginStates: pluginStatesRef.current,
        maxBars: 20, // Show more in tooltip
        displayMode: displayModeRef.current,
      })
      const tooltip = formatTrayTooltip(tooltipBars, pluginsMetaRef.current)
      const updateTooltip = () => setTrayTooltip(tooltip)

      if (style === "bars") {
        renderTrayBarsIcon({
          bars: barsForPreview,
          sizePx,
          style: "bars",
          foregroundColor,
        })
          .then(async (img) => {
            if (!isCurrentUpdate()) return
            await tray.setIcon(img)
            await setTrayIconTemplate(true)
            await setTrayTitle("")
            await updateTooltip()
          })
          .catch((e) => {
            console.error("Failed to update tray icon:", e)
          })
          .finally(() => {
            finalizeUpdate()
          })
        return
      }

      if (!trayProviderId) {
        restoreGaugeIcon()
        return
      }
      lastTrayProviderIdRef.current = trayProviderId

      if (style === "donut") {
        renderTrayBarsIcon({
          bars: providerBars,
          sizePx,
          style: "donut",
          providerIconUrl,
          foregroundColor,
        })
          .then(async (img) => {
            if (!isCurrentUpdate()) return
            await tray.setIcon(img)
            await setTrayIconTemplate(true)
            await setTrayTitle("")
            await updateTooltip()
          })
          .catch((e) => {
            console.error("Failed to update tray icon:", e)
          })
          .finally(() => {
            finalizeUpdate()
          })
        return
      }

      renderTrayBarsIcon({
        bars: providerBars,
        sizePx,
        style: "provider",
        percentText: supportsNativeTrayTitle ? undefined : providerPercentText,
        providerIconUrl,
        foregroundColor,
      })
        .then(async (img) => {
          if (!isCurrentUpdate()) return
          await tray.setIcon(img)
          await setTrayIconTemplate(true)
          await setTrayTitle(providerPercentText)
          await updateTooltip()
        })
        .catch((e) => {
          console.error("Failed to update tray icon:", e)
        })
        .finally(() => {
          finalizeUpdate()
        })
    }, delayMs)
  }, [])

  useEffect(() => {
    const nextIsLinuxTray = isLinuxTrayRuntime()
    isLinuxTrayRef.current = nextIsLinuxTray
    const nextColor = getTrayForegroundColor(themeModeRef.current, nextIsLinuxTray)
    if (nextColor !== trayForegroundColorRef.current) {
      trayForegroundColorRef.current = nextColor
      if (trayReady) {
        scheduleTrayIconUpdate("settings", 0)
      }
    }
  }, [scheduleTrayIconUpdate, trayReady])

  useEffect(() => {
    themeModeRef.current = themeMode
    const nextForegroundColor = getTrayForegroundColor(themeMode, isLinuxTrayRef.current)
    if (nextForegroundColor !== trayForegroundColorRef.current) {
      trayForegroundColorRef.current = nextForegroundColor
      if (trayReady) {
        scheduleTrayIconUpdate("settings", 0)
      }
    }
  }, [themeMode, trayReady, scheduleTrayIconUpdate])

  useEffect(() => {
    if (themeModeRef.current !== "system") return
    if (typeof window === "undefined" || typeof window.matchMedia !== "function") return

    const darkModeMedia = window.matchMedia("(prefers-color-scheme: dark)")
    const onThemeChange = () => {
      const nextColor = getTrayForegroundColor(themeModeRef.current, isLinuxTrayRef.current)
      if (nextColor !== trayForegroundColorRef.current) {
        trayForegroundColorRef.current = nextColor
        if (trayReady) {
          scheduleTrayIconUpdate("settings", 0)
        }
      }
    }

    if ("addEventListener" in darkModeMedia) {
      darkModeMedia.addEventListener("change", onThemeChange)
    } else {
      // @ts-expect-error Legacy API compatibility for older runtimes.
      darkModeMedia.addListener(onThemeChange)
    }

    return () => {
      if ("removeEventListener" in darkModeMedia) {
        darkModeMedia.removeEventListener("change", onThemeChange)
      } else {
        // @ts-expect-error Legacy API compatibility for older runtimes.
        darkModeMedia.removeListener(onThemeChange)
      }
    }
  }, [scheduleTrayIconUpdate, trayReady, themeMode])

  const trayInitializedRef = useRef(false)
  useEffect(() => {
    if (trayInitializedRef.current) return
    let cancelled = false

    ;(async () => {
      try {
        const tray = await TrayIcon.getById("tray")
        if (cancelled) return
        trayRef.current = tray
        trayInitializedRef.current = true

        try {
          trayGaugeIconPathRef.current = await resolveResource("icons/tray-icon.png")
        } catch (e) {
          console.error("Failed to resolve tray gauge icon resource:", e)
        }

        if (cancelled) return
        setTrayReady(true)
      } catch (e) {
        console.error("Failed to load tray icon handle:", e)
      }
    })()

    return () => {
      cancelled = true
    }
  }, [])

  useEffect(() => {
    if (!trayReady) return
    if (!pluginSettings) return
    if (pluginsMeta.length === 0) return
    scheduleTrayIconUpdate("init", 0)
  }, [pluginsMeta.length, pluginSettings, scheduleTrayIconUpdate, trayReady])

  useEffect(() => {
    if (!trayReady) return
    scheduleTrayIconUpdate("settings", 0)
  }, [activeView, menubarIconStyle, scheduleTrayIconUpdate, trayReady])

  useEffect(() => {
    return () => {
      if (trayUpdateTimerRef.current !== null) {
        window.clearTimeout(trayUpdateTimerRef.current)
        trayUpdateTimerRef.current = null
      }
      trayUpdatePendingRef.current = false
      trayUpdateQueuedRef.current = false
      trayUpdateGenerationRef.current += 1
    }
  }, [])

  return {
    scheduleTrayIconUpdate,
    traySettingsPreview,
  }
}
