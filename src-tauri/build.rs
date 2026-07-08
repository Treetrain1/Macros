fn main() {
    build_frontend();
    tauri_build::build();

    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("icons/icon.ico");
        res.set("FileDescription", "Macros");
        res.set("ProductName", "Macros");
        if let Err(e) = res.compile() {
            eprintln!("Failed to embed Windows resources: {e}");
            std::process::exit(1);
        }
    }
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

    let run = |args: &[&str]| {
        let status = std::process::Command::new("pnpm")
            .args(args)
            .current_dir(&ui_dir)
            .status()
            .unwrap_or_else(|e| panic!("failed to run `pnpm {}` in {ui_dir:?}: {e}", args.join(" ")));
        if !status.success() {
            panic!("`pnpm {}` in {ui_dir:?} failed with {status}", args.join(" "));
        }
    };

    run(&["install"]);
    run(&["run", "build"]);
}
