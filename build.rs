fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("res/icons/macros.ico");
        res.set("FileDescription", "Macros");
        res.set("ProductName", "Macros");
        res.set("FileVersion", env!("CARGO_PKG_VERSION"));
        res.set("ProductVersion", env!("CARGO_PKG_VERSION"));
        if let Err(err) = res.compile() {
            eprintln!("Failed to embed Windows resources: {err}");
            std::process::exit(1);
        }
    }
}
