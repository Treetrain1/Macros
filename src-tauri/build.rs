fn main() {
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
