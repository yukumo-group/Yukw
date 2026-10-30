"""Synthesize one sentence for every aq1 preset, aq2 phont, and aq10 base."""

import struct
import sys
import time
from pathlib import Path

from wasmtime import Engine, Instance, Module, Store

TEXT = "きょうはあさからてんきがよくて、そらにはうすぐもがうかんでいました。まどをあけると、とおくのやまからふいてくるかぜがへやのなかにはいってきて、つくえのうえのノートがかるくゆれました。よるになったら、えきまえでともだちとあつまって、あたらしいみせのケーキをたべにいくやくそくをしています。"
print(f"Synthesizing {len(TEXT)} characters...")
# Same order as synth::AQ1_VOICES and synth::AQ2_PHONTS.
AQ1 = ("f1", "f2", "f3", "m1", "m2", "r1", "dvd", "imd1", "jgr")
AQ2 = (
    "aq_yukkuri",
    "aq_defo1",
    "aq_f1c",
    "aq_f3a",
    "aq_huskey",
    "aq_m4b",
    "aq_mf1",
    "aq_rb2",
    "aq_rb3",
    "aq_rm",
    "aq_robo",
    "aq_teto1",
    "ar_f4",
    "ar_m5",
    "ar_mf2",
    "ar_rm3",
    "aq_momo1",
)
# AquesTalk10 基本素片 F1E/F2E/M1E. Preset 0 reads base and the prosody fields.
# Speed is 50-300 on every engine; 100 is normal. aq10 fsc 100 is 16 kHz.
AQ10_BASES = (("f1e", 0), ("f2e", 1), ("m1e", 2))


def main() -> None:
    out_dir = Path(sys.argv[1]) if len(sys.argv) > 1 else Path.cwd() / "out"
    out_dir.mkdir(parents=True, exist_ok=True)

    # The static-engine module has one memory and no imports.
    engine = Engine()
    store = Store(engine)
    instance = Instance(store, Module(engine, Path("yukw.wasm").read_bytes()), [])
    exports = instance.exports(store)
    memory, synthesize, free = exports["memory"], exports["aq_synthesize"], exports["aq_free"]
    try:
        exports["aq2_builtin_phont"]
    except KeyError:
        raise SystemExit(
            "yukw.wasm has no aq2_builtin_phont export, so every aq2 preset uses the "
            "built-in phont and aq10 fsc is read from the wrong offset. Rebuild with "
            "`just wasm` and copy aq_wasm.wasm here as yukw.wasm."
        )

    text = TEXT.encode()
    text_at = 65_536
    param_at = (text_at + len(text) + 16 + 3) & ~3
    out_ptr = param_at + 64
    out_len = out_ptr + 4
    memory.write(store, text, text_at)

    def write_wav(name: str, version: int, preset: int, base: int = 0) -> None:
        # version, preset, speed, phont_data, base, volume, pitch, accent, lmd, fsc
        # aq10 custom leaves preset at 0 and fills the prosody fields.
        prosody = 100 if version == 10 else 0
        param = struct.pack(
            "<10i",
            version,
            preset,
            100,
            0,
            base,
            prosody,
            prosody,
            prosody,
            prosody,
            prosody,
        )
        memory.write(store, param, param_at)
        started = time.perf_counter()
        code = synthesize(store, text_at, len(text), param_at, 0, out_ptr, out_len)
        elapsed = time.perf_counter() - started
        if code:
            raise RuntimeError(f"{name}: aq_synthesize returned {code}")
        ptr, length = struct.unpack("<II", bytes(memory.read(store, out_ptr, out_len + 4)))
        if not ptr or length < 4:
            raise RuntimeError(f"{name}: empty wav")
        wav = bytes(memory.read(store, ptr, ptr + length))
        free(store, ptr)
        if wav[:4] != b"RIFF":
            raise RuntimeError(f"{name}: not a RIFF wav")
        path = out_dir / f"{name}.wav"
        path.write_bytes(wav)
        per_char = elapsed / len(TEXT)
        print(
            f"{path} ({len(wav)} bytes, {elapsed * 1000:.1f} ms, "
            f"{per_char * 1000:.3f} ms/char)"
        )

    for preset, name in enumerate(AQ1):
        write_wav(f"aq1-{name}", 1, preset)
    for preset, name in enumerate(AQ2):
        write_wav(f"aq2-{name}", 2, preset)
    for name, base in AQ10_BASES:
        write_wav(f"aq10-{name}", 10, 0, base)


if __name__ == "__main__":
    main()
