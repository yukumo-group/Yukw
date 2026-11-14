**English** · [中文](README.zh.md)

# Yukw

Yukw is a Rust/WebAssembly runtime for AquesTalk and the evolution of
[aqtk-shim](https://github.com/yukumo-group/aqtk-shim): a WebAssembly-native
AquesTalk1/2/10 compatibility layer for the web and embedded runtimes. It links
the C engines from
[aquestalk-wasm](https://github.com/yukumo-group/aquestalk-wasm) — built there
as relocatable archives — with Rust code through `wasm-ld` (the rust-lld that
cargo uses for `wasm32-unknown-unknown`) into **a single wasm module**:

- **0 imports**
- **exactly one linear memory** (browsers do not support multi-memory, so this
  is what makes the module portable)
- **no embind/Emscripten runtime** and no per-voice memory

## Relationship to aqtk-shim

Yukw is also the evolution of
[aqtk-shim](https://github.com/yukumo-group/aqtk-shim). Both are unified
AquesTalk1/2/10 compatibility layers and share the same parameter model, preset
numbering, and error semantics: the core `AqSynthParam` fields, `AqVersion`
(1/2/10), `Aq1Preset`/`Aq2Preset`/`Aq10Preset`, and the `AQ1_*`/`AQ2_*`/`AQ10_*`
error names.

aqtk-shim targets the desktop and loads native Windows/macOS/Linux libraries at
runtime; Yukw targets wasm and merges the same engines into one Rust module at
link time:

| | aqtk-shim | Yukw |
| --- | --- | --- |
| Runtime | native Windows / macOS / Linux | any wasm32 runtime, browsers |
| Engine source | official SDK dynamic libraries | relocatable archives from aquestalk-wasm |
| Shape | `Aqsh` shared library + `Aqsh.h` | `aq_wasm.wasm` + `aq_wasm.h` (0 imports, 1 memory) |
| Voice loading | runtime directory / `phont/` lookup | static merge at link time |
| Licensing | `AqShim_Init(.env)`, `DevKey`/`UserKey` | evaluation engines; `AqShim_Init` not provided yet |
| Kanji2Koe | none | separate `kanji2koe` module |

Yukw can therefore be seen as the WebAssembly successor to aqtk-shim:
parameter, preset, and error semantics are aligned, but the wasm entry point is
`aq_synthesize` (see [ABI](#abi)), not a binary drop-in for native `Aqsh`.

## Features

| Capability | Module | Main exports |
| --- | --- | --- |
| AquesTalk1/2/10 speech synthesis | `aq-wasm` | `aq_synthesize`, `aq_free`, `aq_error_message`, `aq_set_clock` |
| Direct engine API | `aq-wasm` | `aq1_*`, `aq2_*`, `aq10_*`, `aqk2k_*`, `aqusr_*` |
| Kanji/kana mixed text to kana | `kanji2koe` | `aqk_load`, `aqk_convert`, `aqk_free` |
| Text-to-reading abstraction | `koe` | `ToKoe`, `Error` |

## Layout

```
Yukw/
├── crates/
│   ├── aq-wasm/      # wasm32 synthesis runtime; links the five engine archives
│   ├── kanji2koe/    # standalone Kanji2Koe module; links only libaqk2k.a
│   ├── synth/        # AqSynthParam, versions, presets, error messages
│   ├── koe/          # ToKoe trait
│   ├── character/    # placeholder crate (only the sample add right now)
│   └── script/       # placeholder crate (only the sample add right now)
├── examples/         # Python/wasmtime example
├── justfile
└── Cargo.toml
```

`aq-wasm` and `kanji2koe` are both `cdylib`s; for host builds they are ordinary
Rust rlibs. `build.rs` links the C engine archives only when
`target_arch = "wasm32"`.

## Submodules and wasm dependencies

**This repository has no Git submodules and no `.gitmodules`.** All Rust
dependencies live under `crates/` or come from crates.io. The wasm-related
dependencies fall into three groups:

### 1. Engine archives linked at build time

The engines are not vendored as wasm files. The sibling `aquestalk-wasm`
repository builds **relocatable archives** (`.a` files containing `.o` objects)
and rust-lld links them into the Rust module:

| Archive | Engine | Used by |
| --- | --- | --- |
| `libaq1.a` | AquesTalk1 (9 voices) | `aq-wasm` |
| `libaq2.a` | AquesTalk2 | `aq-wasm` |
| `libaq10.a` | AquesTalk10 | `aq-wasm` |
| `libaqk2k.a` | AqKanji2Koe | `aq-wasm`, `kanji2koe` |
| `libaqusr.a` | AqUsrDic | `aq-wasm` |
| `libaqhost.a` | C-side string/stdio/time/trap | both |
| `exports.txt` | public symbol list (`--export=`) | `aq-wasm/build.rs` |

The default directory is `../aquestalk-wasm/dist/reloc` in a sibling checkout;
set `AQTK_RELOC_DIR` to override it. `aq-wasm/build.rs` links all five engines
with `--whole-archive`; `kanji2koe/build.rs` links only `libaqk2k.a`. On the
Rust side, [host_alloc.rs](crates/aq-wasm/src/host_alloc.rs) provides C
`malloc/free/calloc/realloc` on top of Rust's global allocator.

### 2. Runtime data

- AqKanji2Koe's `lookup.bin`, `aqdic.bin`, and `aq_user.dic` are **licensed
  data and are not in the repository**. The host writes them into wasm memory at
  runtime and calls `aqk_load`; the `kanji2koe` tests use `AQK2K_DIC_DIR` to
  point at the dictionary directory (or at
  `engines/aqk2k/aqk2k_lnx/aq_dic` in a sibling aquestalk-wasm checkout).
- AquesTalk2's phont files are compiled into `libaq2.a`. A call selects one
  with `preset`, or passes custom bytes through `phont_data`. There is no
  phont path.

### 3. Optional local `vendor/` (gitignored)

`vendor/` contains wasm modules from the old Emscripten build. The runtime
does not use them. It is not a submodule, does not participate in the build,
and does not need to be committed.

## Building

Requirements:

- Rust with the `wasm32-unknown-unknown` target
- the LLVM wasm32 toolchain used to build aquestalk-wasm (`clang`, `wasm-ld`,
  `llvm-ar`)
- optional: `just`, Node 18+, Python 3 with wasmtime

```bash
rustup target add wasm32-unknown-unknown

# 1. Build the relocatable engine archives first (sibling checkout)
git clone https://github.com/yukumo-group/aquestalk-wasm.git
cd aquestalk-wasm
./scripts/build-reloc.sh --no-test

# 2. Build Yukw
cd ../Yukw
just wasm        # aq_wasm.wasm + aq_wasm.h in target/.../release/
just kanji2koe   # target/wasm32-unknown-unknown/release/kanji2koe.wasm
```

Without `just`:

```bash
cargo build -p aq-wasm --target wasm32-unknown-unknown --release --lib
cargo build -p kanji2koe --target wasm32-unknown-unknown --release --lib
```

If the two repositories are not siblings:

```bash
AQTK_RELOC_DIR=/path/to/aquestalk-wasm/dist/reloc \
  cargo build -p aq-wasm --target wasm32-unknown-unknown --release --lib
```

Host-side check: `just check` or `cargo check --workspace`.

## ABI

The wasm ABI is documented in
[`crates/aq-wasm/include/aq_wasm.h`](crates/aq-wasm/include/aq_wasm.h).
`just wasm` copies that header next to
`target/wasm32-unknown-unknown/release/aq_wasm.wasm` as `aq_wasm.h`.

### `aq-wasm`

```c
int aq_synthesize(uint32_t text, uint32_t text_len, uint32_t param,
                  uint32_t phont_len, uint32_t out_ptr, uint32_t out_len);
void aq_free(uint32_t ptr);
```

`param` is 40 bytes: 10 `i32` fields (`AqSynthParam` in `aq_wasm.h`):

```
version, preset, speed, phont_data, base, volume, pitch, accent, lmd, fsc
```

- `version`: `1` (AquesTalk1), `2` (AquesTalk2), or `10` (AquesTalk10)
- V1: `preset` 0-8 selects f1/f2/f3/m1/m2/r1/dvd/imd1/jgr
- V2: `preset` 0-16 selects an inlined phont (`aq_yukkuri` through `ar_rm3`,
  then `aq_momo1`). A non-null `phont_data` (+ `phont_len`) overrides `preset`
- V10: `preset` 1-7 selects a built-in preset; 0 uses the custom `base...fsc`
  fields
- Returns 0 and writes the WAV pointer/length on success; returns a negative
  code on failure. Release the WAV with `aq_free`.

The module also exports `malloc/free`, `aq_has_function`, `aq_error_message`,
`aq_static_engines` (the number of linked engines, 5), and the direct APIs of
all five engines (see `aq_wasm.h`).

### `kanji2koe`

```c
int aqk_load(uint32_t lookup, uint32_t lookup_len,
             uint32_t system, uint32_t system_len,
             uint32_t user, uint32_t user_len);
int aqk_convert(int32_t script, uint32_t text, uint32_t text_len,
                uint32_t out_ptr, uint32_t out_len);
void aqk_free(uint32_t ptr);
```

`script = 1` means Japanese. Call `aqk_load` before `aqk_convert`. `malloc/free`
are exported as well; the host uses them for dictionary and input buffers.

## Examples

```bash
# Build and copy the module
just wasm
cp target/wasm32-unknown-unknown/release/aq_wasm.wasm examples/yukw.wasm
cp target/wasm32-unknown-unknown/release/aq_wasm.h examples/

# Python + wasmtime: one WAV per aq1 preset, aq2 phont, and aq10 base
cd examples
./.venv/bin/python main.py          # or: uv run main.py
```

## Testing

```bash
cargo test --workspace
```

- The wasm tests in `aq-wasm` and `kanji2koe` build the wasm32 module and check
  **0 imports / one memory**.
- Tests are skipped when the engine archives are missing; set `AQTK_RELOC_DIR`
  to provide them.
- The `kanji2koe` conversion test also needs dictionaries; set `AQK2K_DIC_DIR`
  or it is skipped.

## Known limitations

- AquesTalk1, AquesTalk10, and AqKanji2Koe are evaluation builds: the ナ row and
  the マ row map to ヌ.
- AquesTalk2 preset phonts are compiled into the module. A phont that is not
  one of those presets is passed as `phont_data`.
- `kanji2koe` loads its dictionaries at runtime; the module no longer embeds the
  9 MB dictionary.
- `character` and `script` are still placeholder crates.
- The old multi-memory/embind merge path has been removed. `vendor/` is unused.
- There is no aqtk-shim `AqShim_Init`/`.env` license loading. The wasm parameter
  block is the 10 fields `version` through `fsc` and does not include
  `DevKey`/`UserKey`. The current engines are evaluation builds; use a
  licensed build for production behavior.

## License

AquesTalk and its voice libraries, phonts, and dictionaries are commercial
software from AQUEST Corp. The code in this repository is for interoperability
research only and does not grant any right to use, distribute, or commercially
run AquesTalk data. Obtain a license from AQUEST for production behavior.
