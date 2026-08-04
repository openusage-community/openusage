use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const EXTENSION_UUID: &str = "openusage-anchor@openusage.app";
const MANIFEST_FILE: &str = ".openusage-anchor.json";
const MANIFEST_CONTENT: &str = r#"{"owner":"openusage","version":1}"#;
const EXTENSION_METADATA: &str = include_str!("../resources/gnome-anchor-extension/metadata.json");
const EXTENSION_SOURCE: &str = include_str!("../resources/gnome-anchor-extension/extension.js");

#[derive(Debug, PartialEq, Eq)]
enum InstallStatus {
    Created,
    Updated,
    Unchanged,
    Unmanaged,
}

pub(crate) fn install_if_gnome_session() {
    if !is_gnome_session() {
        return;
    }

    let Some(extensions_dir) = std::env::var_os("HOME")
        .map(|home| Path::new(&home).join(".local/share/gnome-shell/extensions"))
    else {
        log::warn!("OpenUsage GNOME anchor: HOME is not set");
        return;
    };

    match install_owned_extension(&extensions_dir) {
        Ok(InstallStatus::Created) => {
            log::info!("OpenUsage GNOME anchor installed; log out and back in once to activate it")
        }
        Ok(InstallStatus::Updated | InstallStatus::Unchanged) => reload_extension(),
        Ok(InstallStatus::Unmanaged) => log::warn!(
            "OpenUsage GNOME anchor: refusing to overwrite user-owned extension {}",
            EXTENSION_UUID
        ),
        Err(error) => log::warn!("OpenUsage GNOME anchor: installation failed: {}", error),
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

fn reload_extension() {
    let _ = Command::new("gnome-extensions")
        .args(["disable", EXTENSION_UUID])
        .output();

    match Command::new("gnome-extensions")
        .args(["enable", EXTENSION_UUID])
        .output()
    {
        Ok(output) if output.status.success() => {
            log::info!("OpenUsage GNOME anchor reloaded");
        }
        Ok(output) => log::warn!(
            "OpenUsage GNOME anchor: reload failed (status {:?}): {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).trim()
        ),
        Err(error) => log::warn!(
            "OpenUsage GNOME anchor: cannot run gnome-extensions: {}",
            error
        ),
    }
}

fn install_owned_extension(extensions_dir: &Path) -> io::Result<InstallStatus> {
    let extension_dir = extensions_dir.join(EXTENSION_UUID);
    let existed = extension_dir.exists();
    if existed {
        let manifest = fs::read_to_string(extension_dir.join(MANIFEST_FILE));
        if !matches!(manifest.as_deref(), Ok(content) if content.contains(r#""owner":"openusage""#))
        {
            return Ok(InstallStatus::Unmanaged);
        }
        if matches!(manifest.as_deref(), Ok(MANIFEST_CONTENT))
            && matches!(
                fs::read_to_string(extension_dir.join("metadata.json")).as_deref(),
                Ok(EXTENSION_METADATA)
            )
            && matches!(
                fs::read_to_string(extension_dir.join("extension.js")).as_deref(),
                Ok(EXTENSION_SOURCE)
            )
        {
            return Ok(InstallStatus::Unchanged);
        }
    }

    fs::create_dir_all(extensions_dir)?;
    let staging_dir = extensions_dir.join(format!(
        ".{EXTENSION_UUID}.staging-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must be after Unix epoch")
            .as_nanos()
    ));
    fs::create_dir(&staging_dir)?;
    let result = (|| {
        fs::write(staging_dir.join("metadata.json"), EXTENSION_METADATA)?;
        fs::write(staging_dir.join("extension.js"), EXTENSION_SOURCE)?;
        fs::write(staging_dir.join(MANIFEST_FILE), MANIFEST_CONTENT)?;
        replace_managed_extension(&extension_dir, &staging_dir)
    })();

    if result.is_err() && staging_dir.exists() {
        let _ = fs::remove_dir_all(staging_dir);
    }
    result.map(|()| {
        if existed {
            InstallStatus::Updated
        } else {
            InstallStatus::Created
        }
    })
}

fn replace_managed_extension(extension_dir: &Path, staging_dir: &Path) -> io::Result<()> {
    if !extension_dir.exists() {
        return fs::rename(staging_dir, extension_dir);
    }

    let parent = extension_dir
        .parent()
        .expect("extension directory must have a parent");
    let backup_dir = parent.join(format!(
        ".{EXTENSION_UUID}.previous-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must be after Unix epoch")
            .as_nanos()
    ));
    fs::rename(extension_dir, &backup_dir)?;
    if let Err(error) = fs::rename(staging_dir, extension_dir) {
        let _ = fs::rename(&backup_dir, extension_dir);
        return Err(error);
    }
    fs::remove_dir_all(backup_dir)
}

#[cfg(test)]
mod tests {
    use super::{
        EXTENSION_SOURCE, EXTENSION_UUID, InstallStatus, MANIFEST_FILE, install_owned_extension,
    };
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_dir(name: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("openusage-anchor-{name}-{nanos}"))
    }

    #[test]
    fn extension_tracks_openusage_and_keeps_secondary_clicks() {
        assert!(EXTENSION_SOURCE.contains("Main.panel.statusArea"));
        assert!(EXTENSION_SOURCE.contains("Clutter.BUTTON_PRIMARY"));
        assert!(EXTENSION_SOURCE.contains("/v1/linux-panel/open"));
        assert!(!EXTENSION_SOURCE.contains("Clutter.BUTTON_SECONDARY"));
    }

    #[test]
    fn creates_owned_extension_in_user_directory() {
        let root = test_dir("create");
        let extensions_dir = root.join("extensions");

        assert_eq!(
            install_owned_extension(&extensions_dir).expect("install extension"),
            InstallStatus::Created
        );

        let extension_dir = extensions_dir.join(EXTENSION_UUID);
        assert!(extension_dir.join("metadata.json").is_file());
        assert!(extension_dir.join("extension.js").is_file());
        assert!(extension_dir.join(".openusage-anchor.json").is_file());
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn refreshes_owned_extension_with_an_old_manifest() {
        let root = test_dir("refresh");
        let extensions_dir = root.join("extensions");
        let extension_dir = extensions_dir.join(EXTENSION_UUID);
        fs::create_dir_all(&extension_dir).expect("create old extension");
        fs::write(extension_dir.join("extension.js"), "old source").expect("write old source");
        fs::write(
            extension_dir.join(MANIFEST_FILE),
            r#"{"owner":"openusage","version":0}"#,
        )
        .expect("write old manifest");

        assert_eq!(
            install_owned_extension(&extensions_dir).expect("refresh extension"),
            InstallStatus::Updated
        );

        assert_eq!(
            fs::read_to_string(extension_dir.join("extension.js")).expect("read source"),
            EXTENSION_SOURCE
        );
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn leaves_unmanaged_extension_directory_untouched() {
        let root = test_dir("unmanaged");
        let extensions_dir = root.join("extensions");
        let extension_dir = extensions_dir.join(EXTENSION_UUID);
        fs::create_dir_all(&extension_dir).expect("create user extension");
        fs::write(extension_dir.join("extension.js"), "user source").expect("write user source");

        assert_eq!(
            install_owned_extension(&extensions_dir).expect("inspect user extension"),
            InstallStatus::Unmanaged
        );
        assert_eq!(
            fs::read_to_string(extension_dir.join("extension.js")).expect("read user source"),
            "user source"
        );
        fs::remove_dir_all(root).expect("remove test directory");
    }
}
