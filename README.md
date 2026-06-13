<div align="center">

[![OpenUsage logo](public/icon.png)](https://github.com/openusage-community/openusage)

# OpenUsage Community

 _Made by developers for developers_

</div>

Track all your AI coding subscriptions in one place.

OpenUsage Community is an independent, community-maintained continuation of the original [OpenUsage](https://github.com/robinebers/openusage) project.

The goal of this fork is to continue the cross-platform Tauri-based direction with a strong focus on Linux support, while keeping macOS support and leaving room for future Windows support.

See your usage at a glance from your menu bar or system tray. No digging through dashboards.

![OpenUsage Community Screenshot](banner.png)

## Project Status

OpenUsage Community is currently focused on:

* Linux support
* Stable AppImage, `.deb`, and `.rpm` releases
* System tray behavior across different Linux desktop environments
* Keeping macOS support working
* Preserving the lightweight Tauri-based architecture
* Community-driven maintenance and provider contributions

This project is independent from the original OpenUsage project. It exists because the original project is moving toward a Swift/macOS-first direction, while this fork continues the cross-platform approach.

## Download

[**Download the latest release**](https://github.com/openusage-community/openusage/releases/latest)

Available builds:

* macOS Apple Silicon
* macOS Intel
* Linux AppImage
* Linux `.deb`
* Linux `.rpm`

The app auto-updates where supported. Install once and you're set.

## Linux Notes

OpenUsage Community runs in the system tray.

On desktops without StatusNotifierItem/AppIndicator support, such as GNOME without the AppIndicator extension, a left-click on the tray icon may not open the panel. Use the tray menu's **Show Stats** entry or the global shortcut instead.

On Wayland, panel positioning is best-effort and depends on the compositor. The panel appears under the tray icon where the desktop environment allows it.

Reading credentials stored in the system keyring requires `secret-tool`, usually provided by the `libsecret` or `libsecret-tools` package.

## Install on Linux

Grab the asset for your distro from the [latest release](https://github.com/openusage-community/openusage/releases/latest).

Replace the version in the examples below if a newer release is available.

### Fedora / RHEL

`dnf` pulls in the dependencies automatically:

```sh
sudo dnf install https://github.com/openusage-community/openusage/releases/download/v0.6.24/OpenUsage-0.6.24-1.x86_64.rpm
```

### Debian / Ubuntu

```sh
curl -LO https://github.com/openusage-community/openusage/releases/download/v0.6.24/OpenUsage_0.6.24_amd64.deb
sudo apt install ./OpenUsage_0.6.24_amd64.deb
```

### Any Linux distro

Use the AppImage for a portable build:

```sh
curl -L -o OpenUsage.AppImage https://github.com/openusage-community/openusage/releases/download/v0.6.24/OpenUsage_0.6.24_amd64.AppImage
chmod +x OpenUsage.AppImage
./OpenUsage.AppImage
```

After installing via `.rpm` or `.deb`, launch **OpenUsage Community** from your app menu. The app starts in the system tray.

If your desktop environment has no tray support, install an AppIndicator or StatusNotifier extension, or open the panel with the global shortcut.

Runtime dependencies are handled automatically by `.rpm` and `.deb` packages:

* `webkit2gtk-4.1`
* `gtk3`
* AppIndicator library
* `secret-tool` for providers that read from the system keyring

## What It Does

OpenUsage Community lives in your menu bar or system tray and shows how much of your AI coding subscriptions you have used.

Progress bars, badges, clear labels. No mental math required.

* **One glance.** All your AI tools in one panel.
* **Always up to date.** Refreshes automatically on a schedule you pick.
* **Global shortcut.** Toggle the panel from anywhere with a customizable keyboard shortcut.
* **Lightweight.** Opens quickly and stays out of your way.
* **Plugin-based.** New providers can be added without changing the whole app.
* **[Local HTTP API](docs/local-http-api.md).** Other apps can read your usage data from `127.0.0.1:6736`.
* **[Proxy support](docs/proxy.md).** Route provider HTTP requests through a SOCKS5 or HTTP proxy.

## Supported Providers

* [**Amp**](docs/providers/amp.md) / free tier, bonus, credits
* [**Antigravity**](docs/providers/antigravity.md) / all models
* [**Claude**](docs/providers/claude.md) / session, weekly, extra usage, local token usage with ccusage
* [**Codex**](docs/providers/codex.md) / session, weekly, reviews, credits
* [**Copilot**](docs/providers/copilot.md) / premium, chat, completions
* [**Cursor**](docs/providers/cursor.md) / credits, total usage, auto usage, API usage, on-demand, CLI auth
* [**Factory / Droid**](docs/providers/factory.md) / standard, premium tokens
* [**Gemini**](docs/providers/gemini.md) / pro, flash, workspace/free/paid tier
* [**Grok**](docs/providers/grok.md) / credits used, plan, pay-as-you-go cap
* [**JetBrains AI Assistant**](docs/providers/jetbrains-ai-assistant.md) / quota, remaining
* [**Kiro**](docs/providers/kiro.md) / credits, bonus credits, overages
* [**Kimi Code**](docs/providers/kimi.md) / session, weekly
* [**MiniMax**](docs/providers/minimax.md) / coding plan session
* [**OpenCode Go**](docs/providers/opencode-go.md) / 5h, weekly, monthly spend limits
* [**Windsurf**](docs/providers/windsurf.md) / prompt credits, flex credits
* [**Z.ai**](docs/providers/zai.md) / session, weekly, web searches

Community contributions are welcome.

Want a provider that is not listed? [Open an issue.](https://github.com/openusage-community/openusage/issues/new)

## Open Source, Community Driven

OpenUsage Community is maintained as a community project.

The focus is simple:

* reliable Linux support
* practical cross-platform development
* clean provider integrations
* stable releases
* small, reviewable changes

Contributions are welcome, especially:

* Linux desktop fixes
* packaging improvements
* provider plugins
* documentation
* bug reports with clear reproduction steps
* before/after screenshots for UI fixes

Plugins are currently bundled while the plugin API is being developed. The long-term goal is to make providers more flexible so users and contributors can build and load their own integrations.

## How to Contribute

* **Add a provider.** Each provider is implemented as a plugin. See the [Plugin API](docs/plugins/api.md).
* **Fix a bug.** Pull requests are welcome. Provide reproduction steps and screenshots where relevant.
* **Improve Linux support.** Test across GNOME, KDE Plasma, Xfce, Wayland, and X11.
* **Request a feature.** [Open an issue](https://github.com/openusage-community/openusage/issues/new) and explain the use case.

Keep changes focused. Avoid feature creep. Do not submit AI-generated code without reviewing, testing, and documenting it properly.

## Relationship to OpenUsage

OpenUsage Community is based on the original [OpenUsage](https://github.com/robinebers/openusage) project.

This fork is independent and community-maintained. It continues the Tauri-based cross-platform direction, with Linux as the primary focus.

The original project, its name, and its prior work are credited to the original author and contributors.

## Credits

Based on the original [OpenUsage](https://github.com/robinebers/openusage) project by [Robin Ebers](https://github.com/robinebers).

Inspired by [CodexBar](https://github.com/steipete/CodexBar) by [@steipete](https://github.com/steipete).

Thanks to all original OpenUsage contributors and everyone helping improve cross-platform support.

## License

[MIT](LICENSE)

---

<details>
<summary><strong>Build from source</strong></summary>

> **Warning**: The `main` branch may contain unreleased changes. For regular use, prefer tagged releases. Tagged versions are expected to be tested before publishing.

### Linux build prerequisites

Install the system libraries Tauri needs before building.

#### Debian / Ubuntu

```sh
sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev \
  libayatana-appindicator3-dev librsvg2-dev patchelf \
  libsecret-1-dev build-essential
```

#### Fedora

```sh
sudo dnf install -y webkit2gtk4.1-devel gtk3-devel \
  libappindicator-gtk3-devel librsvg2-devel \
  libsecret-devel patchelf
```

### Stack

OpenUsage Community is built with:

* Tauri
* Rust
* TypeScript
* WebKitGTK on Linux

</details>
