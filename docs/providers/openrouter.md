# OpenRouter

Tracks [OpenRouter](https://openrouter.ai) prepaid API credits and optional per-key usage limits.

## Overview

- **Protocol:** REST (plain JSON)
- **Base URL:** `https://openrouter.ai/api/v1`
- **Auth:** API key via `OPENROUTER_API_KEY`
- **Config fallback:** `~/.config/openusage/openrouter.json`
- **Usage unit:** US dollars
- **Reset period:** none. Credits are prepaid balance.

## Setup

1. Create an API key in [OpenRouter keys](https://openrouter.ai/settings/keys).
2. Set `OPENROUTER_API_KEY`.

OpenUsage is a GUI app. A one-off `export ...` in a terminal session will not be visible when you launch OpenUsage from
Spotlight/Launchpad. Persist it, then restart OpenUsage.

zsh (`~/.zshrc`):

```bash
export OPENROUTER_API_KEY="YOUR_API_KEY"
```

fish (universal var):

```fish
set -Ux OPENROUTER_API_KEY "YOUR_API_KEY"
```

Alternative config file:

```json
{
  "apiKey": "YOUR_API_KEY"
}
```

Save it at `~/.config/openusage/openrouter.json`.

3. Enable the OpenRouter plugin in OpenUsage settings.

## Endpoints

### GET /credits

Returns account-level prepaid credits.

#### Headers

| Header        | Required | Value              |
|---------------|----------|--------------------|
| Authorization | yes      | `Bearer <api_key>` |
| Accept        | yes      | `application/json` |

#### Response

```json
{
  "data": {
    "total_credits": 170,
    "total_usage": 43.45
  }
}
```

Used fields:

- `total_credits` - total prepaid credits
- `total_usage` - credits spent
- remaining is calculated as `total_credits - total_usage`

### GET /key

Returns optional metadata for the current API key. If this endpoint fails, OpenUsage still shows account credits.

#### Response

```json
{
  "data": {
    "limit": 40,
    "limit_remaining": 17.75,
    "usage": 22.25,
    "usage_daily": 1.25,
    "usage_weekly": 7.5,
    "usage_monthly": 22.25,
    "is_free_tier": false
  }
}
```

Used fields:

- `is_free_tier` - displays `Free tier` or `Paid`
- `limit` - per-key spend cap; `null` means unlimited
- `limit_remaining` - remaining amount under the key cap
- `usage`, `usage_daily`, `usage_weekly`, `usage_monthly` - key usage summary

## Displayed Lines

| Line      | Description                                      |
|-----------|--------------------------------------------------|
| Credits   | Account credits spent out of total prepaid value |
| Remaining | Account credits left                             |
| Spent     | Account credits spent                            |
| Tier      | Free tier or paid account marker                 |
| Key limit | Per-key spend cap when configured                |
| Key usage | Total, daily, weekly, and monthly key spend      |

## Errors

| Condition     | Message                                                                         |
|---------------|---------------------------------------------------------------------------------|
| No API key    | "No OPENROUTER_API_KEY found. Set env var or ~/.config/openusage/openrouter.json." |
| 401/403       | "OpenRouter API key invalid. Check OPENROUTER_API_KEY."                         |
| 402           | "OpenRouter credits exhausted. Add credits."                                    |
| HTTP error    | "OpenRouter credits request failed (HTTP {status}). Try again later."           |
| Network error | "OpenRouter credits request failed. Check your connection."                     |
| Invalid shape | "OpenRouter credits response changed."                                          |
