# How to Capture Logs for a Bug Report

Use this when OpenUsage is not working and you need to share debug info.

- Audience: non-technical users
- Time: ~2 minutes
- Platform: macOS

## 1) Set log level to Debug

1. In your macOS menu bar, find the OpenUsage icon.
2. Right-click it (or hold `Control` and click).
3. Open `Debug Level`.
4. Select `Debug`.

If OpenUsage does not open at all, skip this step and continue.

## 2) Reproduce the issue once

1. Do the action that fails.
2. Wait for the failure to happen.
3. Stop after 1-2 attempts (enough data, less noise).

## 3) Check recent error logs in OpenUsage

1. Open OpenUsage.
2. Go to `Settings`.
3. Click `Open error logs`.
4. Pick the day when the issue happened.
5. Copy the visible error text into your bug report.

If OpenUsage does not open, continue with the log folder steps below.

## 4) Open the log folder in Finder

1. Open Finder.
2. Press `Shift` + `Command` + `G`.
3. Paste this path:

```text
~/Library/Logs/com.sunstory.openusage
```

4. Press `Enter`.

## 5) Attach log files to your GitHub issue

1. Attach `openusage.log`.
2. If you also see files like `openusage.log.1`, attach those too.
3. Drag the files directly into your issue/comment on GitHub.

## 6) Add this context in the same issue comment

Copy/paste and fill:

```text
What I expected:
What happened instead:
When it happened (local time + timezone):
Which provider was affected (Codex / Claude / Cursor / etc.):
OpenUsage version:
```

## Privacy note

Logs are redacted for common secrets, but still review before sharing in public.

## Optional: switch log level back

After sending logs, set `Debug Level` back to `Error`.
