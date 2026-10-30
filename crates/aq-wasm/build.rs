use std::path::PathBuf;

fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();
    if target.starts_with("wasm32") {
        println!("cargo:rustc-link-arg=--export-memory");
        link_static_engines();
    }
}

/// Link the relocatable AquesTalk archives and the host shims.
///
/// The archives are passed explicitly (not through `-l`) so that
/// `--whole-archive` applies to exactly the five engines.  `libaqhost.a` comes
/// after them and provides the string/stdio/trap helpers; the crate itself
/// provides `malloc`/`free` from `src/host_alloc.rs`.
fn link_static_engines() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dir = std::env::var_os("AQTK_RELOC_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest.join("../../../aquestalk-wasm/dist/reloc"));

    println!("cargo:rerun-if-env-changed=AQTK_RELOC_DIR");

    let engines = [
        "libaq1.a",
        "libaq2.a",
        "libaq10.a",
        "libaqk2k.a",
        "libaqusr.a",
    ];
    for name in engines {
        let path = dir.join(name);
        if !path.exists() {
            panic!(
                "aq-wasm: {} is missing.\n\
                 Run `./scripts/build-reloc.sh` in the aquestalk-wasm checkout, or set \
                 AQTK_RELOC_DIR to the directory that contains the .a files.",
                path.display()
            );
        }
        println!("cargo:rerun-if-changed={}", path.display());
    }
    let host = dir.join("libaqhost.a");
    if !host.exists() {
        panic!(
            "aq-wasm: {} is missing (rebuild the link kit)",
            host.display()
        );
    }
    println!("cargo:rerun-if-changed={}", host.display());

    println!("cargo:rustc-link-arg=--whole-archive");
    for name in engines {
        println!("cargo:rustc-link-arg={}", dir.join(name).display());
    }
    println!("cargo:rustc-link-arg=--no-whole-archive");
    println!("cargo:rustc-link-arg={}", host.display());
    println!("cargo:rustc-link-arg=-zstack-size=8388608");
    println!("cargo:rustc-link-arg=--max-memory=1073741824");

    let exports = dir.join("exports.txt");
    if exports.exists() {
        println!("cargo:rerun-if-changed={}", exports.display());
        let text = std::fs::read_to_string(&exports).expect("read exports.txt");
        for line in text.lines() {
            let Some(name) = line.strip_prefix("--export=") else {
                continue;
            };
            if matches!(name, "malloc" | "free" | "aq_host_abi") {
                continue;
            }
            println!("cargo:rustc-link-arg=--export={name}");
        }
    }
}
