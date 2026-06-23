(function () {
  const BASE_URL = "https://openrouter.ai/api/v1"
  const CREDITS_URL = BASE_URL + "/credits"
  const KEY_URL = BASE_URL + "/key"
  const CONFIG_PATH = "~/.config/openusage/openrouter.json"
  const BRAND_COLOR = "#6467F2"

  function readString(value) {
    if (typeof value !== "string") return null
    const trimmed = value.trim()
    return trimmed ? trimmed : null
  }

  function readNumber(value) {
    if (typeof value === "number") return Number.isFinite(value) ? value : null
    if (typeof value !== "string") return null
    const trimmed = value.trim()
    if (!trimmed) return null
    const n = Number(trimmed)
    return Number.isFinite(n) ? n : null
  }

  function formatDollars(value) {
    const n = readNumber(value)
    if (n === null) return null
    return "$" + n.toFixed(2)
  }

  function loadConfigApiKey(ctx) {
    if (!ctx.host.fs.exists(CONFIG_PATH)) return null
    try {
      const text = ctx.host.fs.readText(CONFIG_PATH)
      const parsed = ctx.util.tryParseJson(text)
      return readString(parsed && parsed.apiKey)
    } catch (e) {
      ctx.host.log.warn("config read failed: " + String(e))
      return null
    }
  }

  function loadApiKey(ctx) {
    const envKey = readString(ctx.host.env.get("OPENROUTER_API_KEY"))
    if (envKey) return envKey
    return loadConfigApiKey(ctx)
  }

  function authHeaders(apiKey) {
    return {
      Authorization: "Bearer " + apiKey,
      Accept: "application/json",
      "User-Agent": "OpenUsage",
    }
  }

  function fetchCredits(ctx, apiKey) {
    let resp
    try {
      resp = ctx.util.request({
        method: "GET",
        url: CREDITS_URL,
        headers: authHeaders(apiKey),
        timeoutMs: 10000,
      })
    } catch (e) {
      ctx.host.log.error("credits request exception: " + String(e))
      throw "OpenRouter credits request failed. Check your connection."
    }

    if (ctx.util.isAuthStatus(resp.status)) {
      throw "OpenRouter API key invalid. Check OPENROUTER_API_KEY."
    }
    if (resp.status === 402) {
      throw "OpenRouter credits exhausted. Add credits."
    }
    if (resp.status < 200 || resp.status >= 300) {
      throw "OpenRouter credits request failed (HTTP " + String(resp.status) + "). Try again later."
    }

    const parsed = ctx.util.tryParseJson(resp.bodyText)
    const data = parsed && typeof parsed === "object" ? parsed.data : null
    const totalCredits = readNumber(data && data.total_credits)
    const totalUsage = readNumber(data && data.total_usage)
    if (totalCredits === null || totalUsage === null) {
      throw "OpenRouter credits response changed."
    }

    return {
      totalCredits,
      totalUsage,
      remaining: Math.max(0, totalCredits - totalUsage),
    }
  }

  function fetchKeyOptional(ctx, apiKey) {
    let resp
    try {
      resp = ctx.util.request({
        method: "GET",
        url: KEY_URL,
        headers: authHeaders(apiKey),
        timeoutMs: 10000,
      })
    } catch (e) {
      ctx.host.log.warn("key request failed: " + String(e))
      return null
    }

    if (resp.status < 200 || resp.status >= 300) {
      ctx.host.log.warn("key request failed: HTTP " + String(resp.status))
      return null
    }

    const parsed = ctx.util.tryParseJson(resp.bodyText)
    const data = parsed && typeof parsed === "object" ? parsed.data : null
    if (!data || typeof data !== "object") {
      ctx.host.log.warn("key response invalid")
      return null
    }
    return data
  }

  function usageText(keyData) {
    const parts = []
    const total = formatDollars(keyData.usage)
    const daily = formatDollars(keyData.usage_daily)
    const weekly = formatDollars(keyData.usage_weekly)
    const monthly = formatDollars(keyData.usage_monthly)

    if (total) parts.push(total + " total")
    if (daily) parts.push(daily + " today")
    if (weekly) parts.push(weekly + " week")
    if (monthly) parts.push(monthly + " month")
    return parts.length ? parts.join(" / ") : null
  }

  function appendKeyLines(ctx, lines, keyData) {
    const tier = keyData.is_free_tier === true ? "Free tier" : keyData.is_free_tier === false ? "Paid" : null
    if (tier) {
      lines.push(ctx.line.badge({ label: "Tier", text: tier, color: BRAND_COLOR }))
    }

    const limit = readNumber(keyData.limit)
    const remaining = readNumber(keyData.limit_remaining)
    if (keyData.limit === null) {
      lines.push(ctx.line.text({ label: "Key limit", value: "Unlimited" }))
    } else if (limit !== null && remaining !== null) {
      lines.push(ctx.line.progress({
        label: "Key limit",
        used: Math.max(0, limit - remaining),
        limit,
        format: { kind: "dollars" },
      }))
    }

    const usage = usageText(keyData)
    if (usage) lines.push(ctx.line.text({ label: "Key usage", value: usage }))

    return tier
  }

  function probe(ctx) {
    const apiKey = loadApiKey(ctx)
    if (!apiKey) {
      throw "No OPENROUTER_API_KEY found. Set env var or ~/.config/openusage/openrouter.json."
    }

    const credits = fetchCredits(ctx, apiKey)
    const lines = [
      ctx.line.progress({
        label: "Credits",
        used: credits.totalUsage,
        limit: credits.totalCredits,
        format: { kind: "dollars" },
      }),
      ctx.line.text({ label: "Remaining", value: "$" + credits.remaining.toFixed(2) }),
      ctx.line.text({ label: "Spent", value: "$" + credits.totalUsage.toFixed(2) }),
    ]

    const keyData = fetchKeyOptional(ctx, apiKey)
    const plan = keyData ? appendKeyLines(ctx, lines, keyData) : null

    return { plan, lines }
  }

  globalThis.__openusage_plugin = { id: "openrouter", probe }
})()
