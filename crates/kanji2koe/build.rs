use std::path::PathBuf;

fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();
    if target.starts_with("wasm32") {
        println!("cargo:rustc-link-arg=--export-memory");
        link_static_engine();
    }
}

/// Link `libaqk2k.a` (AqKanji2Koe) and the host helpers.
fn link_static_engine() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dir = std::env::var_os("AQTK_RELOC_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest.join("../../../aquestalk-wasm/dist/reloc"));

    println!("cargo:rerun-if-env-changed=AQTK_RELOC_DIR");

    let engine = dir.join("libaqk2k.a");
    if !engine.exists() {
        panic!(
            "kanji2koe: {} is missing.\n\
             Run `./scripts/build-reloc.sh` in the aquestalk-wasm checkout, or set \
             AQTK_RELOC_DIR to the directory that contains the .a files.",
            engine.display()
        );
    }
    let host = dir.join("libaqhost.a");
    if !host.exists() {
        panic!(
            "kanji2koe: {} is missing (rebuild the link kit)",
            host.display()
        );
    }
    for path in [&engine, &host] {
        println!("cargo:rerun-if-changed={}", path.display());
    }

    println!("cargo:rustc-link-arg=--whole-archive");
    println!("cargo:rustc-link-arg={}", engine.display());
    println!("cargo:rustc-link-arg=--no-whole-archive");
    println!("cargo:rustc-link-arg={}", host.display());
    println!("cargo:rustc-link-arg=-zstack-size=8388608");
    println!("cargo:rustc-link-arg=--max-memory=1073741824");
}
