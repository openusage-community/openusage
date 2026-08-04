use std::process::Command;

use crate::gnome_extension_override::{self, OverrideStatus};

const APPINDICATOR_UUID: &str = "appindicatorsupport@rgcjonas.gmail.com";
const ZORIN_APPINDICATOR_UUID: &str = "zorin-appindicator@zorinos.com";
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

    let Some(user_extensions_dir) = std::env::var_os("HOME")
        .map(|home| std::path::PathBuf::from(home).join(".local/share/gnome-shell/extensions"))
    else {
        log::warn!("GNOME window anchor: HOME is not set");
        return;
    };
    let system_extensions_dir = std::path::Path::new("/usr/share/gnome-shell/extensions");

    for uuid in [ZORIN_APPINDICATOR_UUID, APPINDICATOR_UUID] {
        match gnome_extension_override::prepare_user_override(
            system_extensions_dir,
            &user_extensions_dir,
            uuid,
            patch_indicator_source,
        ) {
            Ok(OverrideStatus::NotFound) => continue,
            Ok(OverrideStatus::Unchanged) => return,
            Ok(OverrideStatus::Changed) => {
                reload_extension(uuid);
                return;
            }
            Ok(OverrideStatus::UnmanagedUserCopy) => {
                log::warn!(
                    "GNOME window anchor: refusing to overwrite user-owned extension {}",
                    uuid
                );
                return;
            }
            Err(error) => {
                log::warn!(
                    "GNOME window anchor: failed to prepare user override for {}: {}",
                    uuid,
                    error
                );
                return;
            }
        }
    }

    log::warn!("GNOME window anchor: supported AppIndicator extension file not found");
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

fn reload_extension(uuid: &str) {
    let _ = Command::new("gnome-extensions")
        .args(["disable", uuid])
        .output();

    match Command::new("gnome-extensions")
        .args(["enable", uuid])
        .output()
    {
        Ok(output) if output.status.success() => {
            log::info!(
                "GNOME extension {} reloaded with OpenUsage window anchor",
                uuid
            );
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            log::warn!(
                "GNOME window anchor: failed to reload {} (status {:?}): {}",
                uuid,
                output.status.code(),
                stderr.trim()
            );
        }
        Err(error) => {
            log::warn!(
                "GNOME window anchor: failed to run gnome-extensions for {}: {}",
                uuid,
                error
            );
        }
    }
}

fn patch_indicator_source(original: &str) -> Result<String, String> {
    let mut patched = remove_existing_patch(&original)
        .replace(PATCH_CALL, "")
        .replace(PATCH_INIT_CALL, "")
        .replace(PATCH_INIT_SINGLE_CALL, "")
        .replace(PATCH_DESTROY_CALL, "");

    if !patched.contains("import GLib from 'gi://GLib';") {
        patched = replace_required(
            &patched,
            "import Gio from 'gi://Gio';\n",
            "import Gio from 'gi://Gio';\nimport GLib from 'gi://GLib';\n",
            "GLib import",
        )?;
    }

    patched = replace_in_indicator(
        &patched,
        "    isReady() {",
        &format!("{PATCH_METHOD}    isReady() {{"),
        "IndicatorStatusIcon isReady method",
    )?;
    patched = replace_in_indicator(
        &patched,
        "        this._showIfReady();\n",
        &format!("        this._showIfReady();\n{PATCH_INIT_CALL}"),
        "IndicatorStatusIcon initialization",
    )?;
    patched = replace_in_indicator(
        &patched,
        "    _onDestroy() {\n        if (this._menuClient) {",
        &format!("    _onDestroy() {{\n{PATCH_DESTROY_CALL}        if (this._menuClient) {{"),
        "IndicatorStatusIcon destroy handler",
    )?;
    let button_handler_start = "    vfunc_button_press_event(event) {\n";
    let wait_double_click = "        if (this._waitDoubleClickPromise)\n            this._waitDoubleClickPromise.cancel();\n\n";
    patched = replace_in_indicator(
        &patched,
        &format!("{button_handler_start}{wait_double_click}"),
        &format!("{button_handler_start}{wait_double_click}{PATCH_CALL}"),
        "IndicatorStatusIcon left-click handler",
    )?;

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

    Ok(patched)
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

fn replace_required(
    content: &str,
    needle: &str,
    replacement: &str,
    description: &str,
) -> Result<String, String> {
    content
        .contains(needle)
        .then(|| content.replacen(needle, replacement, 1))
        .ok_or_else(|| format!("unsupported extension structure: missing {description}"))
}

fn replace_in_indicator(
    content: &str,
    needle: &str,
    replacement: &str,
    description: &str,
) -> Result<String, String> {
    let marker = "export const IndicatorStatusIcon";
    let start = content.find(marker).ok_or_else(|| {
        "unsupported extension structure: missing IndicatorStatusIcon".to_string()
    })?;
    let (prefix, indicator) = content.split_at(start);
    let replaced = replace_required(indicator, needle, replacement, description)?;
    Ok(format!("{prefix}{replaced}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn supported_indicator_source() -> String {
        r#"import Gio from 'gi://Gio';
export const IndicatorStatusIcon = GObject.registerClass(
class IndicatorStatusIcon extends BaseStatusIcon {
    _init(indicator) {
        this._indicator = indicator;
        this._showIfReady();
    }

    _onDestroy() {
        if (this._menuClient) {
        }
    }

    isReady() {
        return true;
    }

    vfunc_button_press_event(event) {
        if (this._waitDoubleClickPromise)
            this._waitDoubleClickPromise.cancel();

    }
});
"#
        .to_string()
    }

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
    fn patcher_injects_anchor_only_into_indicator_class() {
        let patched = patch_indicator_source(&supported_indicator_source()).expect("patch source");

        assert!(patched.contains("import GLib from 'gi://GLib';"));
        assert!(patched.contains("this._openUsageAnchorRetryCount = 0;"));
        assert!(patched.contains("_openUsageAnchorStopTracking();"));
        assert!(patched.contains("_openUsageAnchorHandleButtonPress(event)"));
    }

    #[test]
    fn patcher_replaces_its_previous_patch_without_duplicate_handlers() {
        let once = patch_indicator_source(&supported_indicator_source()).expect("first patch");
        let twice = patch_indicator_source(&once).expect("second patch");

        assert_eq!(twice.matches("_openUsageAnchorStartTracking();").count(), 2);
    }

    #[test]
    fn patcher_rejects_unknown_indicator_structure() {
        let source =
            supported_indicator_source().replace("    vfunc_button_press_event(event) {\n", "");
        let error = patch_indicator_source(&source).expect_err("reject unknown structure");

        assert!(error.contains("left-click handler"));
    }
}
