import { beforeEach, describe, expect, it, vi } from "vitest"
import { makeCtx } from "../test-helpers.js"

const CREDITS_URL = "https://openrouter.ai/api/v1/credits"
const KEY_URL = "https://openrouter.ai/api/v1/key"
const CONFIG_PATH = "~/.config/openusage/openrouter.json"

const loadPlugin = async () => {
  await import("./plugin.js")
  return globalThis.__openusage_plugin
}

function mockEnvKey(ctx, key) {
  ctx.host.env.get.mockImplementation((name) => (name === "OPENROUTER_API_KEY" ? key : null))
}

function mockConfigKey(ctx, key) {
  ctx.host.fs.writeText(CONFIG_PATH, JSON.stringify({ apiKey: key }))
}

function mockOpenRouter(ctx, options = {}) {
  const credits = options.credits ?? {
    data: {
      total_credits: 170,
      total_usage: 43.45,
    },
  }
  const key = options.key ?? {
    data: {
      limit: 40,
      limit_remaining: 17.75,
      usage: 22.25,
      usage_daily: 1.25,
      usage_weekly: 7.5,
      usage_monthly: 22.25,
      is_free_tier: false,
    },
  }

  ctx.host.http.request.mockImplementation((req) => {
    if (req.url === CREDITS_URL) {
      if (options.creditsThrows) throw new Error("ECONNREFUSED")
      return {
        status: options.creditsStatus ?? 200,
        headers: {},
        bodyText: options.creditsBodyText ?? JSON.stringify(credits),
      }
    }
    if (req.url === KEY_URL) {
      if (options.keyThrows) throw new Error("ECONNRESET")
      return {
        status: options.keyStatus ?? 200,
        headers: {},
        bodyText: options.keyBodyText ?? JSON.stringify(key),
      }
    }
    return { status: 404, headers: {}, bodyText: "{}" }
  })
}

describe("openrouter plugin", () => {
  beforeEach(() => {
    delete globalThis.__openusage_plugin
    vi.resetModules()
  })

  it("throws when env and config are missing", async () => {
    const ctx = makeCtx()
    const plugin = await loadPlugin()

    expect(() => plugin.probe(ctx)).toThrow(
      "No OPENROUTER_API_KEY found. Set env var or ~/.config/openusage/openrouter.json."
    )
  })

  it("uses OPENROUTER_API_KEY before config file", async () => {
    const ctx = makeCtx()
    mockEnvKey(ctx, "env-key")
    mockConfigKey(ctx, "config-key")
    mockOpenRouter(ctx)

    const plugin = await loadPlugin()
    plugin.probe(ctx)

    expect(ctx.host.http.request.mock.calls[0][0].headers.Authorization).toBe("Bearer env-key")
    expect(ctx.host.http.request.mock.calls[1][0].headers.Authorization).toBe("Bearer env-key")
  })

  it("uses config fallback when env is missing", async () => {
    const ctx = makeCtx()
    mockConfigKey(ctx, "config-key")
    mockOpenRouter(ctx)

    const plugin = await loadPlugin()
    plugin.probe(ctx)

    expect(ctx.host.http.request.mock.calls[0][0].headers.Authorization).toBe("Bearer config-key")
    expect(ctx.host.http.request.mock.calls[1][0].headers.Authorization).toBe("Bearer config-key")
  })

  it("maps account credits and key limit into display lines", async () => {
    const ctx = makeCtx()
    mockEnvKey(ctx, "test-key")
    mockOpenRouter(ctx)

    const plugin = await loadPlugin()
    const result = plugin.probe(ctx)

    expect(result.plan).toBe("Paid")
    expect(result.lines).toEqual([
      {
        type: "progress",
        label: "Credits",
        used: 43.45,
        limit: 170,
        format: { kind: "dollars" },
      },
      { type: "text", label: "Remaining", value: "$126.55" },
      { type: "text", label: "Spent", value: "$43.45" },
      { type: "badge", label: "Tier", text: "Paid", color: "#6467F2" },
      {
        type: "progress",
        label: "Key limit",
        used: 22.25,
        limit: 40,
        format: { kind: "dollars" },
      },
      { type: "text", label: "Key usage", value: "$22.25 total / $1.25 today / $7.50 week / $22.25 month" },
    ])
  })

  it("shows free tier and unlimited key limit", async () => {
    const ctx = makeCtx()
    mockEnvKey(ctx, "test-key")
    mockOpenRouter(ctx, {
      key: {
        data: {
          limit: null,
          limit_remaining: null,
          usage: 4.5,
          usage_daily: 0.5,
          usage_weekly: 2,
          usage_monthly: 4.5,
          is_free_tier: true,
        },
      },
    })

    const plugin = await loadPlugin()
    const result = plugin.probe(ctx)

    expect(result.plan).toBe("Free tier")
    expect(result.lines.find((line) => line.label === "Tier")).toEqual({
      type: "badge",
      label: "Tier",
      text: "Free tier",
      color: "#6467F2",
    })
    expect(result.lines.find((line) => line.label === "Key limit")).toEqual({
      type: "text",
      label: "Key limit",
      value: "Unlimited",
    })
  })

  it("returns account credits when key endpoint fails", async () => {
    const ctx = makeCtx()
    mockEnvKey(ctx, "test-key")
    mockOpenRouter(ctx, { keyStatus: 500 })

    const plugin = await loadPlugin()
    const result = plugin.probe(ctx)

    expect(result.lines.map((line) => line.label)).toEqual(["Credits", "Remaining", "Spent"])
    expect(ctx.host.log.warn).toHaveBeenCalledWith("key request failed: HTTP 500")
  })

  it("throws on credits auth failure", async () => {
    const ctx = makeCtx()
    mockEnvKey(ctx, "test-key")
    mockOpenRouter(ctx, { creditsStatus: 401 })

    const plugin = await loadPlugin()
    expect(() => plugin.probe(ctx)).toThrow("OpenRouter API key invalid. Check OPENROUTER_API_KEY.")
  })

  it("throws on exhausted credits status", async () => {
    const ctx = makeCtx()
    mockEnvKey(ctx, "test-key")
    mockOpenRouter(ctx, { creditsStatus: 402 })

    const plugin = await loadPlugin()
    expect(() => plugin.probe(ctx)).toThrow("OpenRouter credits exhausted. Add credits.")
  })

  it("throws on credits HTTP error", async () => {
    const ctx = makeCtx()
    mockEnvKey(ctx, "test-key")
    mockOpenRouter(ctx, { creditsStatus: 500 })

    const plugin = await loadPlugin()
    expect(() => plugin.probe(ctx)).toThrow("OpenRouter credits request failed (HTTP 500). Try again later.")
  })

  it("throws on credits network error", async () => {
    const ctx = makeCtx()
    mockEnvKey(ctx, "test-key")
    mockOpenRouter(ctx, { creditsThrows: true })

    const plugin = await loadPlugin()
    expect(() => plugin.probe(ctx)).toThrow("OpenRouter credits request failed. Check your connection.")
  })

  it("throws on invalid credits JSON", async () => {
    const ctx = makeCtx()
    mockEnvKey(ctx, "test-key")
    mockOpenRouter(ctx, { creditsBodyText: "not-json" })

    const plugin = await loadPlugin()
    expect(() => plugin.probe(ctx)).toThrow("OpenRouter credits response changed.")
  })

  it("throws when required credits fields are missing", async () => {
    const ctx = makeCtx()
    mockEnvKey(ctx, "test-key")
    mockOpenRouter(ctx, { credits: { data: { total_credits: 170 } } })

    const plugin = await loadPlugin()
    expect(() => plugin.probe(ctx)).toThrow("OpenRouter credits response changed.")
  })
})
