use std::path::PathBuf;
use std::process::Command;

const APPINDICATOR_UUID: &str = "appindicatorsupport@rgcjonas.gmail.com";
const INDICATOR_FILE: &str = "indicatorStatusIcon.js";
const PATCH_START: &str = "    // OpenUsage window anchor patch start";
const PATCH_END: &str = "    // OpenUsage window anchor patch end";
const PATCH_INIT_SINGLE_CALL: &str = "        this._openUsageAnchorStartTracking();\n";
const PATCH_INIT_CALL: &str = r#"        this._openUsageAnchorStartTracking();
        this._openUsageAnchorRetryCount = 0;
        this._openUsageAnchorRetrySourceId = GLib.timeout_add(
            GLib.PRIORITY_DEFAULT,
            250,
            () => {
                if (this._openUsageAnchorSourceId || this._openUsageAnchorRetryCount >= 40) {
                    this._openUsageAnchorRetrySourceId = 0;
                    return GLib.SOURCE_REMOVE;
                }

                this._openUsageAnchorRetryCount++;
                this._openUsageAnchorStartTracking();
                return GLib.SOURCE_CONTINUE;
            });
"#;
const PATCH_DESTROY_CALL: &str = "        this._openUsageAnchorStopTracking();\n\n";
const PATCH_CALL: &str = "        if (this._openUsageAnchorHandleButtonPress(event))\n            return Clutter.EVENT_STOP;\n\n";
const PATCH_TRAY_BUTTON_RELEASE: &str = "        if (this._openUsageAnchorHandleButtonRelease(event))\n            return Clutter.EVENT_STOP;\n\n";
const PATCH_TRAY_BUTTON_PRESS: &str = "        if (this._openUsageAnchorHandleButtonPress(event))\n            return Clutter.EVENT_STOP;\n\n";
const PATCH_METHOD: &str = r#"    // OpenUsage window anchor patch start
    _openUsageAnchorIsOpenUsage() {
        const title = String(this._indicator?.title ?? '').toLowerCase();
        const accessibleName = String(this.get_accessible_name?.() ?? '').toLowerCase();
        const id = String(this._indicator?.id ?? '').toLowerCase();
        const uniqueId = String(this._indicator?.uniqueId ?? this.uniqueId ?? '').toLowerCase();
        const menuPath = String(this._indicator?.menuPath ?? '').toLowerCase();
        const commandLine = String(this._indicator?.commandLine ?? '').toLowerCase();
        const wmClass = String(this._icon?.wm_class ?? this._icon?.wmClass ?? '').toLowerCase();
        const wmClassInstance = String(this._icon?.wm_class_instance ?? '').toLowerCase();
        const iconName = String(this._icon?.name ?? '').toLowerCase();
        const iconTitle = String(this._icon?.title ?? '').toLowerCase();
        const markers = [
            title,
            accessibleName,
            id,
            uniqueId,
            menuPath,
            commandLine,
            wmClass,
            wmClassInstance,
            iconName,
            iconTitle,
        ];
        return markers.some((value) => value.includes('openusage'));
    }

    _openUsageAnchorBody() {
        const actor = this._box ?? this._icon ?? this;
        const [x, y] = actor.get_transformed_position();
        const [width, height] = actor.get_transformed_size();
        if (![x, y, width, height].every(Number.isFinite))
            return null;

        return JSON.stringify({
            centerX: x + width / 2,
            bottomY: y + height,
        });
    }

    _openUsageAnchorPost(path) {
        if (!this._openUsageAnchorIsOpenUsage())
            return false;

        const body = this._openUsageAnchorBody();
        if (!body)
            return false;

        const request = [
            `POST ${path} HTTP/1.1`,
            'Host: 127.0.0.1:6736',
            'Content-Type: application/json',
            `Content-Length: ${body.length}`,
            'Connection: close',
            '',
            body,
        ].join('\r\n');

        const client = new Gio.SocketClient();
        client.connect_to_host_async('127.0.0.1', 6736, null, (_client, result) => {
            try {
                const connection = client.connect_to_host_finish(result);
                const output = new Gio.DataOutputStream({
                    base_stream: connection.output_stream,
                });
                output.put_string(request, null);
                output.close(null);
                connection.close(null);
            } catch (e) {
                Util.Logger.warn(`OpenUsage anchor request failed: ${e}`);
            }
        });

        return true;
    }

    _openUsageAnchorHandleButtonPress(event) {
        if (event.get_button() !== Clutter.BUTTON_PRIMARY)
            return false;

        if (!this._openUsageAnchorIsOpenUsage())
            return false;

        this._openUsageAnchorPost('/v1/linux-panel/open');
        return true;
    }

    _openUsageAnchorHandleButtonRelease(event) {
        if (event.get_button() !== Clutter.BUTTON_PRIMARY)
            return false;

        if (!this._openUsageAnchorIsOpenUsage())
            return false;

        this._openUsageAnchorPost('/v1/linux-panel/open');
        return true;
    }

    _openUsageAnchorStartTracking() {
        if (!this._openUsageAnchorIsOpenUsage() || this._openUsageAnchorSourceId)
            return;

        this._openUsageAnchorPost('/v1/linux-panel/anchor');
        this._openUsageAnchorSourceId = GLib.timeout_add_seconds(
            GLib.PRIORITY_DEFAULT,
            1,
            () => {
                this._openUsageAnchorPost('/v1/linux-panel/anchor');
                return GLib.SOURCE_CONTINUE;
            });
    }

    _openUsageAnchorStopTracking() {
        if (this._openUsageAnchorRetrySourceId) {
            GLib.Source.remove(this._openUsageAnchorRetrySourceId);
            this._openUsageAnchorRetrySourceId = 0;
        }

        if (!this._openUsageAnchorSourceId)
            return;

        GLib.Source.remove(this._openUsageAnchorSourceId);
        this._openUsageAnchorSourceId = 0;
    }
    // OpenUsage window anchor patch end

"#;

pub(crate) fn install_if_gnome_session() {
    if !is_gnome_session() {
        return;
    }

    let Some(indicator_file) = appindicator_file() else {
        log::warn!("GNOME window anchor: AppIndicator extension file not found");
        return;
    };

    let patched = match patch_appindicator_file(&indicator_file) {
        Ok(patched) => patched,
        Err(error) => {
            log::warn!("GNOME window anchor: patch failed: {}", error);
            return;
        }
    };

    if !patched {
        return;
    }

    let _ = Command::new("gnome-extensions")
        .args(["disable", APPINDICATOR_UUID])
        .output();

    match Command::new("gnome-extensions")
        .args(["enable", APPINDICATOR_UUID])
        .output()
    {
        Ok(output) if output.status.success() => {
            log::info!("GNOME AppIndicator extension reloaded with OpenUsage window anchor");
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            log::warn!(
                "GNOME window anchor: AppIndicator reload failed with status {:?}: {}",
                output.status.code(),
                stderr.trim()
            );
        }
        Err(error) => {
            log::warn!(
                "GNOME window anchor: failed to run gnome-extensions: {}",
                error
            );
        }
    }
}

fn is_gnome_session() -> bool {
    [
        "XDG_CURRENT_DESKTOP",
        "DESKTOP_SESSION",
        "GNOME_SHELL_SESSION_MODE",
    ]
    .iter()
    .filter_map(|key| std::env::var(key).ok())
    .any(|value| value.to_ascii_lowercase().contains("gnome"))
}

fn appindicator_file() -> Option<PathBuf> {
    let home_file = std::env::var_os("HOME").map(|home| {
        PathBuf::from(home)
            .join(".local/share/gnome-shell/extensions")
            .join(APPINDICATOR_UUID)
            .join(INDICATOR_FILE)
    });
    if let Some(path) = home_file.filter(|path| path.exists()) {
        return Some(path);
    }

    let system_file = PathBuf::from("/usr/share/gnome-shell/extensions")
        .join(APPINDICATOR_UUID)
        .join(INDICATOR_FILE);
    system_file.exists().then_some(system_file)
}

fn patch_appindicator_file(path: &PathBuf) -> std::io::Result<bool> {
    let original = std::fs::read_to_string(path)?;
    let mut patched = remove_existing_patch(&original)
        .replace(PATCH_CALL, "")
        .replace(PATCH_INIT_SINGLE_CALL, "")
        .replace(PATCH_INIT_CALL, "")
        .replace(PATCH_DESTROY_CALL, "");

    if !patched.contains("import GLib from 'gi://GLib';") {
        patched = patched.replace(
            "import Gio from 'gi://Gio';\n",
            "import Gio from 'gi://Gio';\nimport GLib from 'gi://GLib';\n",
        );
    }

    if !patched.contains(PATCH_METHOD.trim()) {
        patched = patched.replacen(
            "    isReady() {",
            &format!("{PATCH_METHOD}    isReady() {{"),
            1,
        );
    }
    if !patched.contains("this._openUsageAnchorRetryCount = 0;") {
        let indicator_init_tail = "        this.connect('notify::visible', () => this._updateMenu());\n\n        this._showIfReady();\n    }\n\n    _onDestroy() {";
        patched = patched.replacen(
            indicator_init_tail,
            &format!(
                "        this.connect('notify::visible', () => this._updateMenu());\n\n        this._showIfReady();\n{PATCH_INIT_CALL}    }}\n\n    _onDestroy() {{"
            ),
            1,
        );
    }
    if !patched.contains(PATCH_DESTROY_CALL.trim()) {
        patched = patched.replacen(
            "    _onDestroy() {\n        if (this._menuClient) {",
            &format!("    _onDestroy() {{\n{PATCH_DESTROY_CALL}        if (this._menuClient) {{"),
            1,
        );
    }
    if !patched.contains(PATCH_CALL.trim()) {
        let button_handler_start = "    vfunc_button_press_event(event) {\n";
        let wait_double_click = "        if (this._waitDoubleClickPromise)\n            this._waitDoubleClickPromise.cancel();\n\n";
        let button_handler_with_call =
            format!("{button_handler_start}{wait_double_click}{PATCH_CALL}");
        patched = patched.replace(
            &format!("{button_handler_start}{wait_double_click}"),
            &button_handler_with_call,
        );
    }

    if !patched.contains(PATCH_TRAY_BUTTON_RELEASE.trim()) {
        let original = "        this.connect('button-release-event', (_actor, event) => {\n            this._icon.click(event);\n            this.remove_style_pseudo_class('active');\n            return Clutter.EVENT_PROPAGATE;\n        });";
        let patched_release = format!(
            "        this.connect('button-release-event', (_actor, event) => {{\n{PATCH_TRAY_BUTTON_RELEASE}            this._icon.click(event);\n            this.remove_style_pseudo_class('active');\n            return Clutter.EVENT_PROPAGATE;\n        }});"
        );
        patched = patched.replace(original, &patched_release);
    }

    if !patched.contains(PATCH_TRAY_BUTTON_PRESS.trim()) {
        let original = "        this.connect('button-press-event', (_actor, _event) => {\n            this.add_style_pseudo_class('active');\n            return Clutter.EVENT_PROPAGATE;\n        });";
        let patched_press = format!(
            "        this.connect('button-press-event', (_actor, event) => {{\n            this.add_style_pseudo_class('active');\n{PATCH_TRAY_BUTTON_PRESS}            return Clutter.EVENT_PROPAGATE;\n        }});"
        );
        patched = patched.replace(original, &patched_press);
    }

    if patched == original {
        return Ok(false);
    }

    let backup = path.with_extension("js.openusage-backup");
    if !backup.exists() {
        let _ = std::fs::copy(path, backup);
    }
    std::fs::write(path, patched)?;
    Ok(true)
}

fn remove_existing_patch(content: &str) -> String {
    let mut rest = content.to_string();
    loop {
        let Some(start) = rest.find(PATCH_START) else {
            return rest;
        };
        let Some(end_relative) = rest[start..].find(PATCH_END) else {
            return rest;
        };

        let end = start + end_relative + PATCH_END.len();
        let after = rest[end..].strip_prefix("\n\n").unwrap_or(&rest[end..]);
        rest = format!("{}{}", &rest[..start], after);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchor_body_uses_inner_icon_actor_geometry() {
        assert!(PATCH_METHOD.contains("const actor = this._box ?? this._icon ?? this;"));
        assert!(PATCH_METHOD.contains("actor.get_transformed_position();"));
        assert!(!PATCH_METHOD.contains("const [x, y] = this.get_transformed_position();"));
    }

    #[test]
    fn init_call_retries_until_indicator_identity_is_ready() {
        assert!(PATCH_INIT_CALL.contains("_openUsageAnchorRetrySourceId"));
        assert!(PATCH_INIT_CALL.contains("_openUsageAnchorStartTracking();"));
    }

    #[test]
    fn patcher_replaces_old_single_init_call_with_retry_block() {
        let path = std::env::temp_dir().join(format!(
            "openusage-indicator-test-{}.js",
            std::process::id()
        ));
        let original = format!(
            "import Gio from 'gi://Gio';\n\
export const IndicatorStatusIcon = GObject.registerClass(\n\
class IndicatorStatusIcon extends BaseStatusIcon {{\n\
    _init(indicator) {{\n\
        this.connect('notify::visible', () => this._updateMenu());\n\n\
        this._showIfReady();\n\
{PATCH_INIT_SINGLE_CALL}    }}\n\n\
    _onDestroy() {{\n\
        if (this._menuClient) {{\n\
        }}\n\
    }}\n\n\
    vfunc_event(event) {{\n\
    }}\n\n\
    vfunc_button_press_event(event) {{\n\
        if (this._waitDoubleClickPromise)\n\
            this._waitDoubleClickPromise.cancel();\n\n\
    }}\n\
}});\n"
        );

        std::fs::write(&path, original).expect("write fixture");
        patch_appindicator_file(&path).expect("patch");
        let patched = std::fs::read_to_string(&path).expect("read patched");
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("js.openusage-backup"));

        assert!(patched.contains("this._openUsageAnchorRetryCount = 0;"));
        assert_eq!(
            patched.matches("_openUsageAnchorStartTracking();").count(),
            2
        );
    }
}
