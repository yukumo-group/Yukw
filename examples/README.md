# Examples

See the [repository README](../README.md) for the architecture and build steps.

- `main.py` — synthesize every aq1 preset, aq2 phont, and aq10 base through
  the single-memory `yukw.wasm` module and write one WAV each (Python + wasmtime).
  Prints the synthesis time and the average time per character.

Build the module first:

```bash
cd ..
just wasm
cp target/wasm32-unknown-unknown/release/aq_wasm.wasm examples/yukw.wasm
```
