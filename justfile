# Build target/dist/yukw.wasm from the synth runtime and vendor voices.
wasm:
    cargo build -p aq-wasm --target wasm32-unknown-unknown --lib
    cargo run -p aq-wasm --bin aq-link

# Optimized build of target/dist/yukw.wasm.
wasm-release:
    cargo build -p aq-wasm --target wasm32-unknown-unknown --release --lib
    cargo run -p aq-wasm --release --bin aq-link -- target/wasm32-unknown-unknown/release/aq_wasm.wasm

# Single-memory module with the relocatable engines from ../aquestalk-wasm.
# Build those archives first: ./scripts/build-reloc.sh (in aquestalk-wasm).
# Override their location with AQTK_RELOC_DIR=... if the checkouts are not siblings.
wasm-static:
    cargo build -p aq-wasm --target wasm32-unknown-unknown --release --lib --features static-engines

wasm-static-debug:
    cargo build -p aq-wasm --target wasm32-unknown-unknown --lib --features static-engines
