# Examples

See the [repository README](../README.md) for the architecture and build steps.

- `main.py` — synthesize with aq1/aq2/aq10 through the single-memory
  `yukw.wasm` module and write three WAV files (Python + wasmtime).
- `bench.mjs` — optional benchmark of the unified module, the old
  `vendor/` modules and `aqtk-shim` (Node).

Build the module first:

```bash
cd ..
just wasm
cp target/wasm32-unknown-unknown/release/aq_wasm.wasm examples/yukw.wasm
```
