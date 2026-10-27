"""Synthesize one default sentence with aq1, aq2, and aq10."""

import struct
import sys
from pathlib import Path

from wasmtime import Config, Engine, Instance, Module, Store

TEXT = "ゆっくりしていってね"
VOICES = (("aq1", 1, 0, None), ("aq2", 2, 0, "aq_yukkuri"), ("aq10", 10, 1, None))
TEXT_AT, PARAM_AT, PHONT_AT, OUT_PTR, OUT_LEN = 65_536, 66_000, 66_100, 90_000, 90_004


def main() -> None:
    out_dir = Path(sys.argv[1]) if len(sys.argv) > 1 else Path.cwd() / "out"
    out_dir.mkdir(parents=True, exist_ok=True)

    config = Config()
    config.wasm_multi_memory = True
    engine = Engine(config)
    store = Store(engine)
    instance = Instance(store, Module(engine, Path("yukw.wasm").read_bytes()), [])
    exports = instance.exports(store)
    memory, synthesize, free = exports["memory"], exports["aq_synthesize"], exports["aq_free"]

    text = TEXT.encode()
    memory.write(store, text, TEXT_AT)
    for name, version, preset, phont in VOICES:
        phont_ptr = 0
        if phont:
            memory.write(store, phont.encode() + b"\0", PHONT_AT)
            phont_ptr = PHONT_AT
        # version, preset, speed, phont, phont_data, base, volume, pitch, accent, lmd, fsc
        memory.write(store, struct.pack("<11I", version, preset, 100, phont_ptr, 0, 0, 0, 0, 0, 0, 0), PARAM_AT)
        code = synthesize(store, TEXT_AT, len(text), PARAM_AT, 0, OUT_PTR, OUT_LEN)
        if code:
            raise RuntimeError(f"{name}: aq_synthesize returned {code}")
        ptr, length = struct.unpack("<II", bytes(memory.read(store, OUT_PTR, OUT_LEN + 4)))
        if not ptr or length < 4:
            raise RuntimeError(f"{name}: empty wav")
        wav = bytes(memory.read(store, ptr, ptr + length))
        free(store, ptr)
        if wav[:4] != b"RIFF":
            raise RuntimeError(f"{name}: not a RIFF wav")
        path = out_dir / f"{name}.wav"
        path.write_bytes(wav)
        print(f"{path} ({len(wav)} bytes)")


if __name__ == "__main__":
    main()
