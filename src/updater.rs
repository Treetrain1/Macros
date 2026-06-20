#![cfg(windows)]

use self_update::backends::github::Update;
use std::path::{Path, PathBuf};
use std::time::Duration;

const REPO_OWNER: &str = "EthanRStokes";
const REPO_NAME: &str = "macros";
// Must match the Windows artifact_name produced by .github/workflows/release.yml.
const ASSET_NAME: &str = "macros-windows-x86_64.exe";

#[derive(Debug, Clone)]
pub(crate) struct UpdateInfo {
    pub(crate) version: String,
}

fn build_updater(current_version: &str) -> Result<Box<dyn self_update::update::ReleaseUpdate>, String> {
    Update::configure()
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .bin_name("macros")
        .bin_path_in_archive(ASSET_NAME)
        .current_version(current_version)
        .show_download_progress(false)
        .show_output(false)
        .no_confirm(true)
        .build()
        .map_err(|err| err.to_string())
}

/// Blocking — call via `tokio::task::spawn_blocking`. `Ok(None)` means already up to date.
pub(crate) fn check_for_update(current_version: &str) -> Result<Option<UpdateInfo>, String> {
    let updater = build_updater(current_version)?;
    let release = updater
        .get_latest_release()
        .map_err(|err| err.to_string())?;

    let is_newer = self_update::version::bump_is_greater(current_version, &release.version)
        .map_err(|err| err.to_string())?;

    Ok(is_newer.then(|| UpdateInfo { version: release.version }))
}

/// Blocking — call via `tokio::task::spawn_blocking`. Downloads the latest release asset and
/// replaces the running exe in place (via `self_replace`, used internally by `self_update`
/// whenever the install path matches the currently running executable).
pub(crate) fn apply_update(current_version: &str) -> Result<PathBuf, String> {
    let updater = build_updater(current_version)?;
    updater.update().map_err(|err| err.to_string())?;
    std::env::current_exe().map_err(|err| err.to_string())
}

/// Spawns `exe_path` as a fresh detached process. Caller is expected to exit the current
/// process immediately afterwards. Retries briefly since the just-replaced exe file can be
/// momentarily locked (e.g. by antivirus real-time scanning).
pub(crate) fn relaunch(exe_path: &Path) -> Result<(), String> {
    let mut last_err = None;
    for attempt in 0..3 {
        if attempt > 0 {
            std::thread::sleep(Duration::from_millis(100));
        }
        match std::process::Command::new(exe_path).spawn() {
            Ok(_) => return Ok(()),
            Err(err) => last_err = Some(err.to_string()),
        }
    }
    Err(last_err.unwrap_or_else(|| "failed to relaunch after update".to_string()))
}
