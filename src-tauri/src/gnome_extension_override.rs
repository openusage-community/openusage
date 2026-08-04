use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const MANIFEST_FILE: &str = ".openusage-anchor.json";
const MANIFEST_VERSION: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OverrideStatus {
    NotFound,
    Unchanged,
    Changed,
    UnmanagedUserCopy,
}

#[derive(Debug, Deserialize, Serialize)]
struct OverrideManifest {
    version: u8,
    source_fingerprint: String,
}

pub(crate) fn prepare_user_override(
    system_extensions_dir: &Path,
    user_extensions_dir: &Path,
    uuid: &str,
    patch: impl Fn(&str) -> Result<String, String>,
) -> io::Result<OverrideStatus> {
    let source_dir = system_extensions_dir.join(uuid);
    let source_indicator = source_dir.join("indicatorStatusIcon.js");
    if !source_indicator.is_file() {
        return Ok(OverrideStatus::NotFound);
    }

    let source_fingerprint = fingerprint_directory(&source_dir)?;
    let user_dir = user_extensions_dir.join(uuid);
    if user_dir.exists() {
        let Some(manifest) = read_manifest(&user_dir) else {
            return Ok(OverrideStatus::UnmanagedUserCopy);
        };
        if manifest.source_fingerprint == source_fingerprint
            && user_dir.join("indicatorStatusIcon.js").is_file()
        {
            return Ok(OverrideStatus::Unchanged);
        }
    }

    fs::create_dir_all(user_extensions_dir)?;
    let staging_dir = unique_sibling(user_extensions_dir, uuid, "staging");
    fs::create_dir(&staging_dir)?;
    let result = (|| {
        copy_directory(&source_dir, &staging_dir)?;
        let indicator = staging_dir.join("indicatorStatusIcon.js");
        let original = fs::read_to_string(&indicator)?;
        let patched =
            patch(&original).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        fs::write(indicator, patched)?;
        write_manifest(&staging_dir, &source_fingerprint)?;
        replace_managed_directory(&user_dir, &staging_dir)
    })();

    if result.is_err() && staging_dir.exists() {
        let _ = fs::remove_dir_all(&staging_dir);
    }
    result.map(|()| OverrideStatus::Changed)
}

fn read_manifest(directory: &Path) -> Option<OverrideManifest> {
    let content = fs::read_to_string(directory.join(MANIFEST_FILE)).ok()?;
    let manifest: OverrideManifest = serde_json::from_str(&content).ok()?;
    (manifest.version == MANIFEST_VERSION).then_some(manifest)
}

fn write_manifest(directory: &Path, source_fingerprint: &str) -> io::Result<()> {
    let manifest = OverrideManifest {
        version: MANIFEST_VERSION,
        source_fingerprint: source_fingerprint.to_string(),
    };
    let content = serde_json::to_string(&manifest).expect("anchor manifest must serialize");
    fs::write(directory.join(MANIFEST_FILE), content)
}

fn replace_managed_directory(user_dir: &Path, staging_dir: &Path) -> io::Result<()> {
    if !user_dir.exists() {
        return fs::rename(staging_dir, user_dir);
    }

    let parent = user_dir
        .parent()
        .expect("extension directory must have a parent");
    let uuid = user_dir
        .file_name()
        .and_then(|name| name.to_str())
        .expect("extension directory must have a valid name");
    let backup_dir = unique_sibling(parent, uuid, "previous");
    fs::rename(user_dir, &backup_dir)?;
    if let Err(error) = fs::rename(staging_dir, user_dir) {
        let _ = fs::rename(&backup_dir, user_dir);
        return Err(error);
    }
    fs::remove_dir_all(backup_dir)
}

fn unique_sibling(parent: &Path, uuid: &str, purpose: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after Unix epoch")
        .as_nanos();
    parent.join(format!(".{uuid}.openusage-{purpose}-{nanos}"))
}

fn copy_directory(source: &Path, destination: &Path) -> io::Result<()> {
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            fs::create_dir(&destination_path)?;
            copy_directory(&source_path, &destination_path)?;
        } else if file_type.is_file() {
            fs::copy(source_path, destination_path)?;
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "GNOME extension contains an unsupported filesystem entry",
            ));
        }
    }
    Ok(())
}

fn fingerprint_directory(directory: &Path) -> io::Result<String> {
    let mut files = BTreeMap::new();
    collect_files(directory, Path::new(""), &mut files)?;
    let mut hasher = Sha256::new();
    for (relative, contents) in files {
        hasher.update(relative.as_bytes());
        hasher.update([0]);
        hasher.update(contents);
        hasher.update([0]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn collect_files(
    directory: &Path,
    relative_dir: &Path,
    files: &mut BTreeMap<String, Vec<u8>>,
) -> io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let relative = relative_dir.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_files(&path, &relative, files)?;
        } else if file_type.is_file() {
            files.insert(relative.to_string_lossy().into_owned(), fs::read(path)?);
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "GNOME extension contains an unsupported filesystem entry",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "openusage-gnome-extension-{name}-{}",
            unique_sibling(Path::new(""), "test", "dir").display()
        ))
    }

    fn create_source(root: &Path, uuid: &str, indicator: &str) -> PathBuf {
        let source = root.join("system").join(uuid);
        fs::create_dir_all(source.join("nested")).expect("create source");
        fs::write(source.join("indicatorStatusIcon.js"), indicator).expect("write indicator");
        fs::write(source.join("nested/helper.js"), "helper").expect("write helper");
        source
    }

    #[test]
    fn creates_patched_user_copy_without_changing_source() {
        let root = test_dir("create");
        let uuid = "zorin-appindicator@zorinos.com";
        let source = create_source(&root, uuid, "source");
        let user_root = root.join("user");

        let status = prepare_user_override(&root.join("system"), &user_root, uuid, |source| {
            Ok(format!("{source}-patched"))
        })
        .expect("prepare override");

        assert_eq!(status, OverrideStatus::Changed);
        assert_eq!(
            fs::read_to_string(source.join("indicatorStatusIcon.js")).unwrap(),
            "source"
        );
        assert_eq!(
            fs::read_to_string(user_root.join(uuid).join("indicatorStatusIcon.js")).unwrap(),
            "source-patched"
        );
        assert!(user_root.join(uuid).join(MANIFEST_FILE).is_file());
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn refreshes_managed_copy_when_system_extension_changes() {
        let root = test_dir("refresh");
        let uuid = "zorin-appindicator@zorinos.com";
        let source = create_source(&root, uuid, "first");
        let user_root = root.join("user");
        let patch = |source: &str| Ok(format!("{source}-patched"));

        prepare_user_override(&root.join("system"), &user_root, uuid, patch).expect("first copy");
        fs::write(source.join("nested/helper.js"), "updated").expect("update source");
        let status = prepare_user_override(&root.join("system"), &user_root, uuid, patch)
            .expect("refresh copy");

        assert_eq!(status, OverrideStatus::Changed);
        assert_eq!(
            fs::read_to_string(user_root.join(uuid).join("nested/helper.js")).unwrap(),
            "updated"
        );
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn leaves_unmanaged_user_copy_untouched() {
        let root = test_dir("unmanaged");
        let uuid = "zorin-appindicator@zorinos.com";
        create_source(&root, uuid, "source");
        let user_dir = root.join("user").join(uuid);
        fs::create_dir_all(&user_dir).expect("create user copy");
        fs::write(user_dir.join("indicatorStatusIcon.js"), "user-owned").expect("write user copy");

        let status =
            prepare_user_override(&root.join("system"), &root.join("user"), uuid, |source| {
                Ok(format!("{source}-patched"))
            })
            .expect("detect unmanaged copy");

        assert_eq!(status, OverrideStatus::UnmanagedUserCopy);
        assert_eq!(
            fs::read_to_string(user_dir.join("indicatorStatusIcon.js")).unwrap(),
            "user-owned"
        );
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn does_not_create_copy_when_patcher_rejects_source() {
        let root = test_dir("invalid");
        let uuid = "zorin-appindicator@zorinos.com";
        create_source(&root, uuid, "unsupported");
        let user_root = root.join("user");

        let error = prepare_user_override(&root.join("system"), &user_root, uuid, |_| {
            Err("unsupported indicator structure".to_string())
        })
        .expect_err("patch should fail");

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(!user_root.join(uuid).exists());
        fs::remove_dir_all(root).expect("remove test directory");
    }
}
