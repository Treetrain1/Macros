//! Enumerates locally installed applications for the "Open App" instruction's
//! picker popup — desktop-app-only concern (unlike the cross-platform
//! `InstructionKind::OpenApp` itself, which just launches whatever `command`
//! string this produced and never re-scans anything), so this lives here
//! rather than in `blockwork-core`.
//!
//! Icon support is currently Linux-only: freedesktop `.desktop` entries name
//! an icon theme lookup key, which resolves fairly reliably to a `.png`/
//! `.svg` file we can inline as a `data:` URI. Windows/macOS listings below
//! are best-effort (name + launch target only, no icon) — resolving a Start
//! Menu shortcut's icon or a `.icns` bundle icon needs real image decoding
//! this app has no other reason to depend on.

pub(crate) struct AppEntry {
    pub(crate) name: String,
    pub(crate) command: String,
    pub(crate) icon: Option<String>,
}

#[cfg(target_os = "linux")]
pub(crate) fn list_apps() -> Vec<AppEntry> {
    linux::list_apps()
}

#[cfg(target_os = "windows")]
pub(crate) fn list_apps() -> Vec<AppEntry> {
    windows::list_apps()
}

#[cfg(target_os = "macos")]
pub(crate) fn list_apps() -> Vec<AppEntry> {
    macos::list_apps()
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
pub(crate) fn list_apps() -> Vec<AppEntry> {
    Vec::new()
}

#[cfg(target_os = "linux")]
mod linux {
    use super::AppEntry;
    use std::collections::HashSet;
    use std::fs;
    use std::path::{Path, PathBuf};

    /// Every `applications/` directory the freedesktop menu spec says to
    /// search, most-specific (user overrides) last so `seen_ids` lets an
    /// earlier, more sensitive-in-priority terms win — but as of writing we
    /// just take the first `.desktop` file we see per id and skip the rest,
    /// which is the opposite; that's fine here since we don't need override
    /// semantics, just "don't list the same app id twice".
    fn application_dirs() -> Vec<PathBuf> {
        let mut dirs = Vec::new();
        match std::env::var("XDG_DATA_DIRS") {
            Ok(v) if !v.is_empty() => dirs.extend(v.split(':').map(|d| PathBuf::from(d).join("applications"))),
            _ => {
                dirs.push(PathBuf::from("/usr/local/share/applications"));
                dirs.push(PathBuf::from("/usr/share/applications"));
            }
        }
        if let Some(data_home) = dirs::data_dir() {
            dirs.push(data_home.join("applications"));
        }
        dirs
    }

    pub(crate) fn list_apps() -> Vec<AppEntry> {
        let mut seen_ids = HashSet::new();
        let mut apps = Vec::new();
        for dir in application_dirs() {
            let Ok(entries) = fs::read_dir(&dir) else { continue };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                    continue;
                }
                let Some(id) = path.file_name().and_then(|n| n.to_str()).map(str::to_string) else { continue };
                if !seen_ids.insert(id) {
                    continue;
                }
                if let Some(app) = parse_desktop_entry(&path) {
                    apps.push(app);
                }
            }
        }
        apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        apps
    }

    /// Reads the handful of keys we care about out of a `.desktop` file's
    /// `[Desktop Entry]` section — a purpose-built scan rather than a general
    /// INI parser, since that's all this needs.
    fn parse_desktop_entry(path: &Path) -> Option<AppEntry> {
        let content = fs::read_to_string(path).ok()?;
        let mut in_main_section = false;
        let mut name = None;
        let mut exec = None;
        let mut icon = None;
        let mut no_display = false;
        let mut hidden = false;
        let mut is_application = true;

        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('[') {
                in_main_section = line == "[Desktop Entry]";
                continue;
            }
            if !in_main_section {
                continue;
            }
            if let Some(v) = line.strip_prefix("Name=") {
                if name.is_none() {
                    name = Some(v.to_string());
                }
            } else if let Some(v) = line.strip_prefix("Exec=") {
                exec = Some(v.to_string());
            } else if let Some(v) = line.strip_prefix("Icon=") {
                icon = Some(v.to_string());
            } else if let Some(v) = line.strip_prefix("NoDisplay=") {
                no_display = v.eq_ignore_ascii_case("true");
            } else if let Some(v) = line.strip_prefix("Hidden=") {
                hidden = v.eq_ignore_ascii_case("true");
            } else if let Some(v) = line.strip_prefix("Type=") {
                is_application = v == "Application";
            }
        }

        if no_display || hidden || !is_application {
            return None;
        }
        let name = name?;
        let command = clean_exec_command(&exec?);
        if command.is_empty() {
            return None;
        }
        let icon = icon.and_then(|i| resolve_icon(&i));
        Some(AppEntry { name, command, icon })
    }

    /// Strips freedesktop field codes (`%f`/`%F`/`%u`/`%U`/etc.) from an
    /// `Exec=` line — those stand for file/URL args a launch from this
    /// picker never has, so they're dropped rather than substituted.
    fn clean_exec_command(exec: &str) -> String {
        let mut result = String::new();
        let mut chars = exec.chars().peekable();
        while let Some(c) = chars.next() {
            if c != '%' {
                result.push(c);
                continue;
            }
            match chars.next() {
                Some('%') => result.push('%'),
                Some('f' | 'F' | 'u' | 'U' | 'd' | 'D' | 'n' | 'N' | 'i' | 'c' | 'k' | 'v' | 'm') | None => {}
                Some(other) => {
                    result.push('%');
                    result.push(other);
                }
            }
        }
        result.trim().to_string()
    }

    /// Resolves a `.desktop` file's `Icon=` value (either an absolute path,
    /// or an icon-theme lookup key) to a `data:` URI, if a usable file was
    /// found for it.
    fn resolve_icon(icon_name: &str) -> Option<String> {
        let path = if Path::new(icon_name).is_absolute() { PathBuf::from(icon_name) } else { find_icon_file(icon_name)? };
        icon_file_to_data_uri(&path)
    }

    /// Best-effort icon-theme lookup: rather than parsing every theme's
    /// `index.theme` to know its real size/scale layout, just probes the
    /// common hicolor-style `<theme>/<size>/apps/<name>.<ext>` layout most
    /// installed themes follow, largest first, plus the older flat
    /// `pixmaps` directory as a fallback.
    fn find_icon_file(name: &str) -> Option<PathBuf> {
        let mut base_dirs: Vec<PathBuf> = Vec::new();
        if let Some(home) = dirs::home_dir() {
            base_dirs.push(home.join(".local/share/icons"));
            base_dirs.push(home.join(".icons"));
        }
        base_dirs.push(PathBuf::from("/usr/share/icons"));
        base_dirs.push(PathBuf::from("/usr/local/share/icons"));

        const THEMES: &[&str] = &["hicolor", "Adwaita", "gnome", "breeze", "Papirus"];
        const SIZES: &[&str] = &["scalable", "256x256", "128x128", "96x96", "64x64", "48x48", "32x32"];
        const EXTS: &[&str] = &["svg", "png"];

        for base in &base_dirs {
            for theme in THEMES {
                for size in SIZES {
                    for ext in EXTS {
                        let candidate = base.join(theme).join(size).join("apps").join(format!("{name}.{ext}"));
                        if candidate.is_file() {
                            return Some(candidate);
                        }
                    }
                }
            }
        }
        for dir in ["/usr/share/pixmaps", "/usr/local/share/pixmaps"] {
            for ext in EXTS {
                let candidate = PathBuf::from(dir).join(format!("{name}.{ext}"));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
        None
    }

    fn icon_file_to_data_uri(path: &Path) -> Option<String> {
        let mime = match path.extension().and_then(|e| e.to_str()) {
            Some("svg") => "image/svg+xml",
            Some("png") => "image/png",
            // .xpm and other legacy formats aren't natively displayable by
            // an <img> tag without decoding/re-encoding first.
            _ => return None,
        };
        let bytes = fs::read(path).ok()?;
        use base64::Engine;
        Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use super::AppEntry;
    use std::path::Path;

    /// Start Menu shortcuts, for the current user and "all users" — no icon
    /// (see module doc comment); `command` is the `.lnk` path itself, which
    /// `runner::open_app`'s `cmd /C start "" <path>` launches directly.
    pub(crate) fn list_apps() -> Vec<AppEntry> {
        let mut dirs = Vec::new();
        if let Ok(program_data) = std::env::var("ProgramData") {
            dirs.push(Path::new(&program_data).join("Microsoft/Windows/Start Menu/Programs"));
        }
        if let Ok(app_data) = std::env::var("APPDATA") {
            dirs.push(Path::new(&app_data).join("Microsoft/Windows/Start Menu/Programs"));
        }

        let mut apps = Vec::new();
        for dir in dirs {
            walk(&dir, &mut apps);
        }
        apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        apps
    }

    fn walk(dir: &Path, out: &mut Vec<AppEntry>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("lnk")) != Some(true) {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else { continue };
            // Skip the noisy "Uninstall X"/"X Website" shortcuts most
            // installers also drop into the Start Menu next to the real app.
            let lower = stem.to_lowercase();
            if lower.starts_with("uninstall") || lower.contains("website") {
                continue;
            }
            out.push(AppEntry { name: stem.to_string(), command: path.to_string_lossy().to_string(), icon: None });
        }
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::AppEntry;
    use std::path::Path;

    /// Top-level `.app` bundles in `/Applications` and `~/Applications` — no
    /// icon (see module doc comment); `command` is the bundle path itself,
    /// which `runner::open_app`'s `open <path>` launches directly.
    pub(crate) fn list_apps() -> Vec<AppEntry> {
        let mut dirs = vec![Path::new("/Applications").to_path_buf()];
        if let Some(home) = dirs::home_dir() {
            dirs.push(home.join("Applications"));
        }

        let mut apps = Vec::new();
        for dir in dirs {
            let Ok(entries) = std::fs::read_dir(&dir) else { continue };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("app") {
                    continue;
                }
                let Some(name) = path.file_stem().and_then(|s| s.to_str()) else { continue };
                apps.push(AppEntry { name: name.to_string(), command: path.to_string_lossy().to_string(), icon: None });
            }
        }
        apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        apps
    }
}
