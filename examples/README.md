# Examples

See the [repository README](../README.md) for the architecture and build steps.

- `main.py` — synthesize every aq1 preset, aq2 phont, and aq10 base through
  the single-memory `yukw.wasm` module and write one WAV each (Python + wasmtime).
  Prints the synthesis time and the average time per character.
- `aq_wasm.h` — ABI companion for the module (copied from
  `crates/aq-wasm/include/aq_wasm.h` by `just wasm`).

Build the module first:

```bash
cd ..
just wasm
cp target/wasm32-unknown-unknown/release/aq_wasm.wasm examples/yukw.wasm
cp target/wasm32-unknown-unknown/release/aq_wasm.h examples/
```
