//! This build script copies the `memory.x` file from the crate root into
//! a directory where the linker can always find it at build time.
//! For many projects this is optional, as the linker always searches the
//! project root directory -- wherever `Cargo.toml` is. However, if you
//! are using a workspace or have a more complicated build setup, this
//! build script becomes required. Additionally, by requesting that
//! Cargo re-run the build script whenever `memory.x` is changed,
//! updating `memory.x` ensures a rebuild of the application with the
//! new memory settings.
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::{env, fs};

use superpattern::transform_pattern;

fn main() {
    memory();
    build_superpattern();
}

fn build_superpattern() {
    println!("cargo:rerun-if-changed=superpattern/patterns");

    for e in fs::read_dir("superpattern/patterns").unwrap().enumerate() {
        let i = e.0;
        let entry = e.1.unwrap();
        let path = entry.path();

        if path.extension().is_some_and(|e| e != "epe") {
            continue;
        }

        let mut raw_json = String::new();
        File::open(&path)
            .expect("Failed to open")
            .read_to_string(&mut raw_json)
            .unwrap();
        let pattern = json::parse(&raw_json.trim_matches(|t: char| !t.is_ascii()))
            .map_err(|e| format!("failed to parse {:?}: {:?}", path, e))
            .unwrap();
        let str = pattern["sources"]["main"]
            .as_str()
            .expect("sources.main was not a string");

        let res = transform_pattern(str);

        _ = File::write_all(
            &mut File::create(format!("superpattern/generated/generated-{}.js", i)).unwrap(),
            res.transformed_pattern.as_bytes(),
        );
    }
}

fn memory() {
    // Put `memory.x` in our output directory and ensure it's
    // on the linker search path.
    let out = &PathBuf::from(env::var_os("OUT_DIR").unwrap());
    File::create(out.join("memory.x"))
        .unwrap()
        .write_all(include_bytes!("memory.x"))
        .unwrap();
    println!("cargo:rustc-link-search={}", out.display());

    // By default, Cargo will re-run a build script whenever
    // any file in the project changes. By specifying `memory.x`
    // here, we ensure the build script is only re-run when
    // `memory.x` is changed.
    println!("cargo:rerun-if-changed=memory.x");

    println!("cargo:rustc-link-arg-bins=--nmagic");
    println!("cargo:rustc-link-arg-bins=-Tlink.x");
    println!("cargo:rustc-link-arg-bins=-Tlink-rp.x");
    println!("cargo:rustc-link-arg-bins=-Tdefmt.x");
}
