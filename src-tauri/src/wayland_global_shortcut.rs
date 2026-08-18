//! Wayland global shortcuts via the XDG `org.freedesktop.portal.GlobalShortcuts` portal.
//!
//! `tauri-plugin-global-shortcut` uses X11 `XGrabKey`, which never fires on Wayland (the
//! compositor owns the keyboard). The portal registers a named shortcut with a preferred
//! trigger; the desktop owns the final binding and emits `Activated` on press.

use std::sync::Mutex;

use tauri::AppHandle;

const SHORTCUT_ID: &str = "toggle-panel";

static PORTAL_TASK: Mutex<Option<tauri::async_runtime::JoinHandle<()>>> = Mutex::new(None);

pub fn is_wayland() -> bool {
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        return true;
    }
    matches!(std::env::var("XDG_SESSION_TYPE"), Ok(value) if value.eq_ignore_ascii_case("wayland"))
}

/// (Re)register the global shortcut through the portal. Pass `None` to disable it.
pub fn configure(app_handle: &AppHandle, shortcut: Option<String>) {
    if let Some(handle) = PORTAL_TASK.lock().expect("portal task lock poisoned").take() {
        handle.abort();
    }

    let Some(shortcut) = shortcut else {
        log::info!("wayland global shortcut disabled");
        return;
    };

    let app_handle = app_handle.clone();
    let handle = tauri::async_runtime::spawn(async move {
        if let Err(error) = run_portal(app_handle, shortcut).await {
            log::warn!("wayland global shortcut portal failed: {error}");
        }
    });
    *PORTAL_TASK.lock().expect("portal task lock poisoned") = Some(handle);
}

async fn run_portal(app_handle: AppHandle, shortcut: String) -> Result<(), ashpd::Error> {
    use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut};
    use futures_util::StreamExt;

    let global_shortcuts = GlobalShortcuts::new().await?;
    let session = global_shortcuts.create_session(Default::default()).await?;

    let preferred = to_portal_trigger(&shortcut);
    let new_shortcut = NewShortcut::new(SHORTCUT_ID, "Show / hide the OpenUsage panel")
        .preferred_trigger(Some(preferred.as_str()));

    global_shortcuts
        .bind_shortcuts(&session, &[new_shortcut], None, Default::default())
        .await?
        .response()?;
    log::info!(
        "wayland global shortcut registered via portal (preferred trigger '{}')",
        preferred
    );

    let mut activated = global_shortcuts.receive_activated().await?;
    while let Some(activation) = activated.next().await {
        if activation.shortcut_id() != SHORTCUT_ID {
            continue;
        }
        log::debug!("wayland global shortcut activated");
        let app = app_handle.clone();
        if let Err(error) = app_handle.run_on_main_thread(move || {
            crate::panel::toggle_panel(&app);
        }) {
            log::warn!("failed to toggle panel from global shortcut: {error}");
        }
    }

    // Session must outlive the listener loop.
    drop(session);
    Ok(())
}

/// Map a Tauri accelerator to the XDG shortcuts trigger syntax. Only a hint; the desktop
/// may override it.
fn to_portal_trigger(shortcut: &str) -> String {
    shortcut
        .split('+')
        .map(|part| match part.trim().to_ascii_lowercase().as_str() {
            "commandorcontrol" | "cmdorctrl" | "control" | "ctrl" => "CTRL".to_string(),
            "shift" => "SHIFT".to_string(),
            "alt" | "option" => "ALT".to_string(),
            "super" | "meta" | "command" | "cmd" | "win" | "logo" => "LOGO".to_string(),
            other => other.to_uppercase(),
        })
        .collect::<Vec<_>>()
        .join("+")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_common_accelerators_to_portal_triggers() {
        assert_eq!(to_portal_trigger("CommandOrControl+Shift+U"), "CTRL+SHIFT+U");
        assert_eq!(to_portal_trigger("Control+Alt+K"), "CTRL+ALT+K");
        assert_eq!(to_portal_trigger("Super+Space"), "LOGO+SPACE");
    }
}
