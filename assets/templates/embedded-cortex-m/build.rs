//! Put `memory.x` on the linker search path and re-run when it changes.

use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by cargo"));
    fs::copy("memory.x", out.join("memory.x")).expect("memory.x sits next to Cargo.toml");
    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rerun-if-changed=memory.x");
}
