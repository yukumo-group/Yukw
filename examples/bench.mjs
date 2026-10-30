// Synthesis speed for a short line and a long passage, on the linked runtime,
// each vendor voice, and aqtk-shim.
//
//   node examples/bench.mjs [--warmup 2] [--iters 10]
//
// Vendor modules get the same marker swap the old linker applied. Every sample,
// including the native shim, is timed here with performance.now. aqtk_bench
// only synthesizes. It is optional until the AquesTalk libraries are staged:
//
//   cmake --preset linux && cmake --build --preset linux --target aqtk_bench

import { spawn } from "node:child_process";
import readline from "node:readline";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const TEXTS = [
  ["short", "ゆっくりしていってね"],
  [
    "long",
    "きょうはあさからてんきがよくて、そらにはうすぐもがうかんでいました。まどをあけると、とおくのやまからふいてくるかぜがへやのなかにはいってきて、つくえのうえのノートがかるくゆれました。よるになったら、えきまえでともだちとあつまって、あたらしいみせのケーキをたべにいくやくそくをしています。",
  ],
];
const CLOCK = 1_793_034_672_000;
const TEXT_AT = 65_536;
const TEXT_LIMIT = 8_192;
const PARAM_AT = TEXT_AT + TEXT_LIMIT;
const PHONT_AT = PARAM_AT + 256;
const OUT_PTR = 98_304;
const OUT_LEN = OUT_PTR + 4;
const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const SHIM_ROOT = path.resolve(ROOT, "../aqtk-shim");

const VOICES = [
  ["aqtk1-f1", "aq1"],
  ["aqtk1-f2", "aq1"],
  ["aqtk1-f3", "aq1"],
  ["aqtk1-m1", "aq1"],
  ["aqtk1-m2", "aq1"],
  ["aqtk1-r1", "aq1"],
  ["aqtk1-dvd", "aq1"],
  ["aqtk1-jgr", "aq1"],
  ["aqtk2", "aq2"],
  ["aqtk10", "aq10"],
];

Date.now = () => CLOCK;

const args = parseArgs(process.argv.slice(2));
const rows = [];

rows.push(...(await benchUnified(args)));
rows.push(...(await benchVendor(args)));
rows.push(...(await benchShim(args)));

printTable(rows);
if (rows.some((row) => row.failed)) {
  process.exitCode = 1;
}

function parseArgs(argv) {
  const out = { warmup: 2, iters: 10, wasm: "", shim: "" };
  for (let i = 0; i < argv.length; i++) {
    const flag = argv[i];
    const value = argv[i + 1];
    if (flag === "--warmup" || flag === "--iters") {
      const n = Number(value);
      if (!Number.isInteger(n) || n < 0) {
        throw new Error(`bad ${flag}`);
      }
      out[flag.slice(2)] = n;
      i++;
    } else if (flag === "--wasm" || flag === "--shim") {
      out[flag.slice(2)] = value;
      i++;
    } else {
      throw new Error(`unknown argument ${flag}`);
    }
  }
  if (out.iters < 1) {
    throw new Error("iters must be positive");
  }
  return out;
}

async function benchUnified(args) {
  const wasmPath = args.wasm || findWasm();
  if (!wasmPath) {
    return [skipped("unified", "yukw.wasm", "not built")];
  }
  const { instance } = await WebAssembly.instantiate(fs.readFileSync(wasmPath));
  const cases = [
    ["aq1", { version: 1, preset: 0 }],
    ["aq2", { version: 2, preset: 0 }],
    ["aq10", { version: 10, preset: 1 }],
  ];
  const out = [];
  for (const [label, sentence] of TEXTS) {
    const encoded = encodeText(sentence);
    for (const [name, spec] of cases) {
      out.push(await measure(`unified ${label}`, name, args, unifiedCall(instance.exports, spec, encoded)));
    }
  }
  return out;
}

function findWasm() {
  for (const rel of ["target/dist/yukw.wasm", "yukw.wasm"]) {
    const full = path.join(ROOT, rel);
    if (fs.existsSync(full)) {
      return full;
    }
  }
  return "";
}

function unifiedCall(exports, spec, encoded) {
  const { memory, aq_synthesize, aq_free } = exports;
  const heap = () => new Uint8Array(memory.buffer);
  return () => {
    const param = new DataView(new ArrayBuffer(44));
    param.setInt32(0, spec.version, true);
    param.setInt32(4, spec.preset, true);
    param.setInt32(8, 100, true);
    heap().set(encoded, TEXT_AT);
    if (spec.phont) {
      heap().set(new TextEncoder().encode(`${spec.phont}\0`), PHONT_AT);
      param.setUint32(12, PHONT_AT, true);
    }
    heap().set(new Uint8Array(param.buffer), PARAM_AT);
    const started = performance.now();
    const code = aq_synthesize(TEXT_AT, encoded.length, PARAM_AT, 0, OUT_PTR, OUT_LEN);
    const ms = performance.now() - started;
    const view = new DataView(memory.buffer);
    const ptr = view.getUint32(OUT_PTR, true);
    const len = view.getUint32(OUT_LEN, true);
    const head = Buffer.from(heap().subarray(ptr, ptr + 4)).toString();
    aq_free(ptr);
    if (code !== 0 || head !== "RIFF" || len < 4) {
      throw new Error(`aq_synthesize returned ${code} (${head || "empty"}, ${len} bytes)`);
    }
    return { ms, bytes: len };
  };
}

async function benchVendor(args) {
  const env = loadEnv(path.join(ROOT, ".env"));
  const find = Buffer.from(env.M4L_FIND_B64 || "", "base64");
  const replace = Buffer.from(env.M4L_REPLACE_B64 || "", "base64");
  const jwt = Buffer.from(env.LICENSE_JWT_B64 || "", "base64").toString("utf8");
  if (find.length !== 32 || replace.length !== 32 || !jwt) {
    return [skipped("vendor", ".env", "missing the marker or license")];
  }
  const byLabel = new Map(TEXTS.map(([label]) => [label, []]));
  for (const [file, kind] of VOICES) {
    const wasmPath = path.join(ROOT, "vendor", `${file}.wasm`);
    const jsPath = path.join(ROOT, "vendor", `${file}.js`);
    if (!fs.existsSync(wasmPath) || !fs.existsSync(jsPath)) {
      for (const [label] of TEXTS) {
        byLabel.get(label).push(skipped(`vendor ${label}`, file, "module not in vendor/"));
      }
      continue;
    }
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), "yukw-bench-"));
    try {
      const patched = patchMarker(fs.readFileSync(wasmPath), find, replace);
      fs.copyFileSync(jsPath, path.join(dir, `${file}.js`));
      fs.writeFileSync(path.join(dir, `${file}.wasm`), patched);
      const loaded = await import(pathToFileURL(path.join(dir, `${file}.js`)).href);
      const api = await loaded.default();
      if (!api.verifyLicense(jwt)) {
        throw new Error("verifyLicense failed");
      }
      let token = api.issueBalanceToken();
      for (const [label, sentence] of TEXTS) {
        byLabel.get(label).push(
          await measure(`vendor ${label}`, file, args, () => voiceSample(api, kind, sentence, () => token, (next) => {
            token = next;
          })),
        );
      }
    } catch (err) {
      for (const [label] of TEXTS) {
        byLabel.get(label).push(failed(`vendor ${label}`, file, err.message || String(err)));
      }
    } finally {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  }
  return TEXTS.flatMap(([label]) => byLabel.get(label));
}

function voiceSample(api, kind, sentence, readToken, writeToken) {
  const started = performance.now();
  const result = callVoice(api, kind, sentence, readToken());
  const ms = performance.now() - started;
  if (typeof result.balanceToken === "string" && result.balanceToken) {
    writeToken(result.balanceToken);
  }
  const data = result.data ? Buffer.from(result.data) : Buffer.alloc(0);
  if (result.errorCode !== 0 || data.subarray(0, 4).toString() !== "RIFF") {
    throw new Error(`synthe returned ${result.errorCode} (${data.length} bytes)`);
  }
  return { ms, bytes: data.length };
}

function callVoice(api, kind, sentence, token) {
  const speed = { spd: 100, vol: 100, fsc: 100, monotone: false };
  if (kind === "aq1") {
    return api.synthe(sentence, speed, token);
  }
  if (kind === "aq2") {
    return api.synthe(sentence, speed, "aq_yukkuri", token);
  }
  return api.synthe(
    { bas: 0, spd: 100, vol: 100, pit: 100, acc: 100, lmd: 100, fsc: 100 },
    sentence,
    false,
    token,
  );
}

function encodeText(sentence) {
  const encoded = new TextEncoder().encode(sentence);
  if (encoded.length > TEXT_LIMIT) {
    throw new Error(`text is ${encoded.length} bytes, limit is ${TEXT_LIMIT}`);
  }
  return encoded;
}

function patchMarker(bytes, find, replace) {
  const out = Buffer.from(bytes);
  let index = 0;
  let hits = 0;
  while (index + find.length <= out.length) {
    if (out.subarray(index, index + find.length).equals(find)) {
      replace.copy(out, index);
      hits += 1;
      index += find.length;
    } else {
      index += 1;
    }
  }
  if (hits === 0) {
    throw new Error("m4l0");
  }
  return out;
}

async function benchShim(args) {
  const bin = args.shim || findShim();
  if (!bin) {
    return TEXTS.map(([label]) => skipped(`shim ${label}`, "aqtk_bench", "not built"));
  }
  const shim = openShim(bin);
  try {
    const ready = await shim.nextLine();
    if (ready !== "ready") {
      throw new Error(ready || "aqtk_bench did not start");
    }
    const out = [];
    for (const [label, sentence] of TEXTS) {
      for (const name of ["aq1", "aq2", "aq10"]) {
        out.push(await measure(`shim ${label}`, name, args, () => shimSample(shim, name, sentence)));
      }
    }
    return out;
  } catch (err) {
    return [failed("shim", "aqtk_bench", err.message || String(err))];
  } finally {
    shim.close();
  }
}

function openShim(bin) {
  const child = spawn(bin, ["--env", path.join(ROOT, ".env")], {
    stdio: ["pipe", "pipe", "inherit"],
  });
  const lines = readline.createInterface({ input: child.stdout });
  const waiters = [];
  lines.on("line", (line) => {
    const waiter = waiters.shift();
    if (waiter) {
      waiter.resolve(line);
    }
  });
  child.on("error", (err) => rejectWaiters(err));
  child.on("exit", (code) => {
    if (waiters.length) {
      rejectWaiters(new Error(`aqtk_bench exited ${code}`));
    }
  });
  return {
    nextLine() {
      return new Promise((resolve, reject) => waiters.push({ resolve, reject }));
    },
    request(name, sentence) {
      const pending = this.nextLine();
      child.stdin.write(`${name}\t${sentence}\n`);
      return pending;
    },
    close() {
      lines.close();
      child.kill();
    },
  };

  function rejectWaiters(err) {
    while (waiters.length) {
      waiters.shift().reject(err);
    }
  }
}

async function shimSample(shim, name, sentence) {
  const started = performance.now();
  const line = await shim.request(name, sentence);
  const ms = performance.now() - started;
  if (!/^\d+$/.test(line)) {
    throw new Error(`aqtk_bench returned ${line || "nothing"}`);
  }
  return { ms, bytes: Number(line) };
}

function findShim() {
  const names = process.platform === "win32" ? ["aqtk_bench.exe"] : ["aqtk_bench"];
  const dirs = [
    "build/linux",
    "build/linux32",
    "build/macos",
    "build/macos-debug",
    "build/x64/Release",
    "build/x64/Debug",
    "build/x86/Release",
    "build/x86/Debug",
  ];
  for (const dir of dirs) {
    for (const name of names) {
      const full = path.join(SHIM_ROOT, dir, name);
      if (fs.existsSync(full)) {
        return full;
      }
    }
  }
  return "";
}

async function measure(group, name, args, call) {
  try {
    for (let i = 0; i < args.warmup; i++) {
      await call();
    }
    const ms = [];
    let bytes = 0;
    for (let i = 0; i < args.iters; i++) {
      const sample = await call();
      ms.push(sample.ms);
      bytes = sample.bytes;
    }
    return rowFrom(group, name, ms, bytes);
  } catch (err) {
    return failed(group, name, err.message || String(err));
  }
}

function rowFrom(group, name, ms, bytes) {
  const sorted = [...ms].sort((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  const median = sorted.length % 2 === 0 ? (sorted[mid - 1] + sorted[mid]) / 2 : sorted[mid];
  const mean = ms.reduce((sum, value) => sum + value, 0) / ms.length;
  return {
    group,
    name,
    median,
    min: sorted[0],
    max: sorted[sorted.length - 1],
    mean,
    bytes,
  };
}

function skipped(group, name, detail) {
  return { group, name, detail };
}

function failed(group, name, detail) {
  return { group, name, detail, failed: true };
}

function printTable(rows) {
  for (const [label, sentence] of TEXTS) {
    console.log(`${label} ${[...sentence].length} chars  ${sentence}`);
  }
  console.log(`warmup ${args.warmup}  iters ${args.iters}`);
  let group = "";
  for (const row of rows) {
    if (row.group !== group) {
      group = row.group;
      console.log(`\n${group}`);
    }
    if (row.detail) {
      console.log(`  ${row.name.padEnd(12)} ${row.detail}`);
      continue;
    }
    console.log(
      `  ${row.name.padEnd(12)} ${fmt(row.median)} ms   min ${fmt(row.min)}   max ${fmt(row.max)}   mean ${fmt(row.mean)}   ${row.bytes} bytes`,
    );
  }
}

function fmt(ms) {
  return ms.toFixed(2).padStart(8);
}

function loadEnv(file) {
  if (!fs.existsSync(file)) {
    return {};
  }
  const out = {};
  for (const line of fs.readFileSync(file, "utf8").split("\n")) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#") || !trimmed.includes("=")) {
      continue;
    }
    const eq = trimmed.indexOf("=");
    out[trimmed.slice(0, eq).trim()] = trimmed.slice(eq + 1).trim();
  }
  return out;
}
