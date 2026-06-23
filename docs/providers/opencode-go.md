# OpenCode Go

> Uses local OpenCode history from SQLite to track observed OpenCode Go spend on this machine.

## Overview

- **Source of truth:** `~/.local/share/opencode/opencode.db`
- **Auth discovery:** `~/.local/share/opencode/auth.json`
- **Provider ID:** `opencode-go`
- **Usage scope:** local observed session spend only

## Detection

The plugin enables when either condition is true:

- `~/.local/share/opencode/auth.json` contains an `opencode-go` entry with a non-empty `key`
- local OpenCode history already contains `opencode-go` sessions with cost data

If neither signal exists, the plugin stays hidden.

## Data Source

OpenUsage reads the local OpenCode SQLite database directly:

```sql
SELECT
  time_updated AS createdMs,
  CAST(cost AS TEXT) AS cost
FROM session
WHERE json_valid(model)
  AND json_extract(model, '$.providerID') = 'opencode-go'
  AND cost > 0
```

Only sessions with a positive cost count. `time_updated` is stored in milliseconds (same unit as `Date.now()`), confirmed from OpenCode's [`packages/core/src/session/sql.ts`](https://github.com/anomalyco/opencode/blob/dev/packages/core/src/session/sql.ts) where `time_created` and `time_updated` use `Date.now()` as their default value. Missing remote or other-device usage is not estimated.

## Limits

OpenUsage uses the current published OpenCode Go plan limits from the official docs:

- `5h`: `$12`
- `Weekly`: `$30`
- `Monthly`: `$60`

Bars show observed local spend as a percentage of those fixed limits and clamp at `100%`.

## Window Rules

- `5h`: rolling last 5 hours from now
- `Weekly`: UTC Monday `00:00` through the next UTC Monday `00:00`
- `Monthly`: inferred subscription-style monthly window using the earliest local OpenCode Go usage timestamp as the anchor

Monthly usage is inferred from local history, not read from OpenCode’s account API. OpenUsage reuses the earliest observed local OpenCode Go usage timestamp as the monthly anchor. If no local history exists yet, it falls back to UTC calendar month boundaries until the first Go usage is recorded.

## Failure Behavior

If auth or prior history already indicates OpenCode Go is in use, but SQLite becomes unreadable or malformed, the provider stays visible and shows a grey `Status: No usage data` badge instead of failing hard.

## Future Compatibility

The public provider identity stays `opencode-go`. If OpenCode later exposes account-truth usage by API key, OpenUsage can swap the backend without changing the provider ID or UI contract.
