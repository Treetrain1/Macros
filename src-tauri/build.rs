fn main() {
    build_frontend();
    // Also embeds the Windows icon + version resource (from tauri.conf.json's
    // productName/bundle.icon) on its own -- a second, manual pass doing the
    // same thing here would emit a duplicate VERSION resource and fail to
    // link with `CVTRES : fatal error CVT1100: duplicate resource`.
    tauri_build::build();
}

// `frontendDist` points at `../ui/dist`, a build output that doesn't exist
// until Vite runs. Plain `cargo build`/`cargo run` (justfile, IDE run
// buttons) never go through the Tauri CLI, so nothing else would trigger
// that build — do it here so the frontend is always fresh regardless of how
// the crate is built.
fn build_frontend() {
    let ui_dir = std::path::Path::new("..").join("ui");

    println!("cargo:rerun-if-changed={}", ui_dir.join("index.html").display());
    println!("cargo:rerun-if-changed={}", ui_dir.join("src").display());
    println!("cargo:rerun-if-changed={}", ui_dir.join("package.json").display());
    println!("cargo:rerun-if-changed={}", ui_dir.join("vite.config.js").display());

    // On Windows, pnpm may be installed as a `.cmd` shim (npm global) or as
    // `pnpm.exe` (WinGet/scoop). Try the common candidates and fall back to
    // the bare name so `Command::new` can resolve it via PATH.
    let pnpm = if cfg!(windows) {
        ["pnpm.cmd", "pnpm.exe", "pnpm"]
            .iter()
            .find(|name| std::process::Command::new("where").arg(name).output().map_or(false, |o| o.status.success()))
            .copied()
            .unwrap_or("pnpm")
    } else {
        "pnpm"
    };

    let run = |args: &[&str]| {
        let mut command = std::process::Command::new(pnpm);
        command.args(args).current_dir(&ui_dir);
        // Sandboxed/offline builds (see packaging/flatpak, which sets
        // BLOCKWORK_PNPM_OFFLINE=1 -- deliberately not FLATPAK, which cef-dll-sys's
        // own build script already keys off to look for CEF at /usr/lib instead
        // of $CEF_PATH) need pnpm to trust the committed lockfile without
        // touching the network -- pnpm 11's env-var config (npm_config_offline,
        // npm_config_trust_lockfile) isn't honored for these, only the CLI flags
        // are, so pass them directly.
        if args[0] == "install" && std::env::var_os("BLOCKWORK_PNPM_OFFLINE").is_some() {
            command.args(["--offline", "--frozen-lockfile", "--trust-lockfile"]);
        }
        let status = command
            .status()
            .unwrap_or_else(|e| panic!("failed to run `pnpm {}` in {ui_dir:?}: {e}", args.join(" ")));
        if !status.success() {
            panic!("`pnpm {}` in {ui_dir:?} failed with {status}", args.join(" "));
        }
    };

    run(&["install"]);
    run(&["run", "build"]);
}
