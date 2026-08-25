use std::env;
use std::path::PathBuf;

fn main() {
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=cbindgen.toml");

    let config = cbindgen::Config::from_file(PathBuf::from(&crate_dir).join("cbindgen.toml"))
        .expect("failed to read cbindgen.toml");

    match cbindgen::Builder::new().with_crate(&crate_dir).with_config(config).generate() {
        Ok(bindings) => {
            bindings.write_to_file(PathBuf::from(&crate_dir).join("include/macros_ffi.h"));
        }
        // A generation failure shouldn't fail the whole build (the header is
        // only needed by the C++ side, not by anything downstream in Cargo);
        // a stale-but-present header is better than none for local iteration.
        Err(err) => {
            println!("cargo:warning=cbindgen header generation failed: {err}");
        }
    }
}
