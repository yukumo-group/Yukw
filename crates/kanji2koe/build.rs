use std::collections::HashMap;
use std::path::Path;

fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();
    if target.starts_with("wasm32") {
        println!("cargo:rustc-link-arg=--export-memory");
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let env_path = root.join(".env");
    println!("cargo:rerun-if-changed={}", env_path.display());
    let map = load_dotenv(&env_path);
    let jwt = required(&map, "LICENSE_JWT_B64");
    println!("cargo:rustc-env=LICENSE_JWT_B64={jwt}");
}

fn required<'a>(map: &'a HashMap<String, String>, key: &str) -> &'a str {
    map.get(key)
        .map(String::as_str)
        .unwrap_or_else(|| panic!("{key} missing from .env"))
}

fn load_dotenv(path: &Path) -> HashMap<String, String> {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    let mut map = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        map.insert(key.trim().to_string(), value.trim().to_string());
    }
    map
}
