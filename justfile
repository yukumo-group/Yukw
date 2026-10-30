# Build the single-memory aq-wasm module: AquesTalk1/2/10 plus the exported
# AqKanji2Koe/AqUsrDic engine entry points. No embind, no multi-memory.
# Build the relocatable archives first in ../aquestalk-wasm:
#   ./scripts/build-reloc.sh
wasm:
    cargo build -p aq-wasm --target wasm32-unknown-unknown --release --lib

# Build the Kanji2Koe reading module. The host loads lookup.bin, aqdic.bin and
# aq_user.dic and calls aqk_load before aqk_convert.
kanji2koe:
    cargo build -p kanji2koe --target wasm32-unknown-unknown --release --lib

# Type-check every crate for the host target.
check:
    cargo check --workspace
