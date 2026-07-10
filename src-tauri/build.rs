fn main() {
    ensure_external_bin_for_tauri_metadata();
    tauri_build::build()
}

fn ensure_external_bin_for_tauri_metadata() {
    let Some(target) = std::env::var_os("TARGET") else {
        return;
    };
    let target = target.to_string_lossy();
    let exe_suffix = if target.contains("windows") {
        ".exe"
    } else {
        ""
    };
    let sidecar =
        std::path::PathBuf::from("binaries").join(format!("openusage-cli-{target}{exe_suffix}"));

    if sidecar.exists() {
        return;
    }

    // Windows support adapted from barramee27/crossusage (MIT): https://github.com/barramee27/crossusage
    // Tauri validates externalBin during `cargo check/test`, before its beforeBuildCommand can
    // build the real sidecar. Create an ignored placeholder for metadata-only Cargo runs.
    if let Some(parent) = sidecar.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let bytes: &[u8] = if target.contains("windows") {
        b""
    } else {
        b"#!/usr/bin/env sh\necho 'openusage-cli sidecar placeholder; run scripts/prepare-cli-sidecar.sh for bundles.' >&2\nexit 1\n"
    };
    let _ = std::fs::write(&sidecar, bytes);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = std::fs::metadata(&sidecar) {
            let mut permissions = metadata.permissions();
            permissions.set_mode(0o755);
            let _ = std::fs::set_permissions(&sidecar, permissions);
        }
    }
}
