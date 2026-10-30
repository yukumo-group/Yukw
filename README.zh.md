[English](README.md) · **中文**

# Yukw

Yukw 是 AquesTalk 的 Rust/WebAssembly 运行时，同时也是
[aqtk-shim](https://github.com/yukumo-group/aqtk-shim) 的进化版：一个面向
wasm、更适合 Web 与嵌入式的 AquesTalk1/2/10 兼容层。它把
[aquestalk-wasm](https://github.com/yukumo-group/aquestalk-wasm) 中的 C 引擎
编译成可重定位归档，再由 `wasm-ld`（cargo 在 `wasm32-unknown-unknown` 上使用的
rust-lld）和 Rust 代码链接成**单个 wasm 模块**：

- **0 imports**
- **恰好 1 个 linear memory**（浏览器不支持 multi-memory，这是可移植的前提）
- 不包含 **embind / Emscripten** 运行时，也不需要每个声库各自的 memory

## 与 aqtk-shim 的关系

Yukw 同时是 [aqtk-shim](https://github.com/yukumo-group/aqtk-shim) 的进化版。
两者都是 AquesTalk1/2/10 的统一兼容层，沿用同一套参数模型、预设编号和错误码
语义：`AqSynthParam` 的核心字段、`AqVersion`（1/2/10）、
`Aq1Preset`/`Aq2Preset`/`Aq10Preset`，以及 `AQ1_*`/`AQ2_*`/`AQ10_*` 错误名。

aqtk-shim 面向桌面，在运行时加载 Windows/macOS/Linux 的原生动态库；Yukw 面向
wasm，把同样的引擎在链接期合并进一个 Rust 模块：

| | aqtk-shim | Yukw |
| --- | --- | --- |
| 运行环境 | Windows / macOS / Linux 原生 | 任意 wasm32 运行时、浏览器 |
| 引擎来源 | 官方 SDK 动态库 | aquestalk-wasm 的可重定位归档 |
| 模块形态 | `Aqsh` 动态库 + `Aqsh.h` | 单个 wasm：0 imports、1 memory |
| 语音库加载 | 运行时按目录/`phont/` 查找 | 链接期静态合并 |
| 许可证 | `AqShim_Init(.env)`、`DevKey`/`UserKey` | 评价版引擎；暂不提供 `AqShim_Init` |
| Kanji2Koe | 无 | 独立的 `kanji2koe` 模块 |

因此可以把 Yukw 看作 aqtk-shim 在 WebAssembly 上的后继：参数/预设/错误语义
对齐，wasm 侧入口是 `aq_synthesize`（见 [ABI](#abi)），而不是原生 `Aqsh` 的
二进制 drop-in。

## 功能

| 能力 | 模块 | 主要导出 |
| --- | --- | --- |
| AquesTalk1/2/10 语音合成 | `aq-wasm` | `aq_synthesize`、`aq_free`、`aq_error_message`、`aq_set_clock` |
| 引擎直通 API | `aq-wasm` | `aq1_*`、`aq2_*`、`aq10_*`、`aqk2k_*`、`aqusr_*` |
| 汉字假名混合文 → 假名 | `kanji2koe` | `aqk_load`、`aqk_convert`、`aqk_free` |
| 文本转读音抽象 | `koe` | `ToKoe`、`Error` |

## 目录

```
Yukw/
├── crates/
│   ├── aq-wasm/      # wasm32 合成运行时；链接 5 个引擎归档
│   ├── kanji2koe/    # 独立的 Kanji2Koe 模块；只链接 libaqk2k.a
│   ├── synth/        # AqSynthParam、版本、预设、错误信息
│   ├── koe/          # ToKoe trait
│   ├── character/    # 占位 crate（目前只有示例 add）
│   └── script/       # 占位 crate（目前只有示例 add）
├── examples/         # Python/wasmtime 示例
├── justfile
└── Cargo.toml
```

`aq-wasm` 和 `kanji2koe` 都是 `cdylib`；host 侧构建时它们是普通的 Rust rlib。
只有 `target_arch = "wasm32"` 时 `build.rs` 才会链接 C 引擎归档。

## 子模块与 wasm 依赖

**本仓库没有 Git submodule，也没有 `.gitmodules`。** 所有 Rust 依赖都在
`crates/` 内或来自 crates.io。wasm 相关依赖分三类：

### 1. 构建期静态链接的引擎归档

引擎不是以 wasm 文件形式 vendored 进来，而是由相邻的
`aquestalk-wasm` 仓库生成**可重定位归档**（`.o` 的 `.a` 包），再由
rust-lld 直接链接进 Rust 模块：

| 归档 | 引擎 | 使用方 |
| --- | --- | --- |
| `libaq1.a` | AquesTalk1（9 声库） | `aq-wasm` |
| `libaq2.a` | AquesTalk2 | `aq-wasm` |
| `libaq10.a` | AquesTalk10 | `aq-wasm` |
| `libaqk2k.a` | AqKanji2Koe | `aq-wasm`、`kanji2koe` |
| `libaqusr.a` | AqUsrDic | `aq-wasm` |
| `libaqhost.a` | C 侧 string/stdio/time/trap | 两者 |
| `exports.txt` | 公开符号清单（`--export=`） | `aq-wasm/build.rs` |

默认目录是相邻 checkout 的 `../aquestalk-wasm/dist/reloc`，可用
`AQTK_RELOC_DIR` 覆盖。`aq-wasm/build.rs` 用 `--whole-archive` 链接全部五个
引擎；`kanji2koe/build.rs` 只链接 `libaqk2k.a`。Rust 侧
[host_alloc.rs](crates/aq-wasm/src/host_alloc.rs) 提供 C 的
`malloc/free/calloc/realloc`，底层是 Rust 全局分配器。

### 2. 运行期数据

- AqKanji2Koe 的 `lookup.bin`、`aqdic.bin`、`aq_user.dic` 是**授权数据，不在
  仓库里**。宿主在运行时写入 wasm 内存并调用 `aqk_load`；`kanji2koe` 的测试用
  `AQK2K_DIC_DIR` 指向词典目录（或相邻 aquestalk-wasm 的
  `engines/aqk2k/aqk2k_lnx/aq_dic`）。
- AquesTalk2 的 phont 文件编译进 `libaq2.a`。调用用 `preset` 选择，或通过
  `phont_data` 传入自定义字节。没有 phont 路径。

### 3. 可选的本地 `vendor/`（gitignored）

`vendor/` 里是旧的 Emscripten 构建的 wasm 模块，运行时不使用。它不是
submodule，不参与构建，也不需要提交。

## 构建

需要：

- Rust + `wasm32-unknown-unknown` target
- 构建 aquestalk-wasm 时的 LLVM wasm32 工具链（`clang`、`wasm-ld`、`llvm-ar`）
- 可选：`just`、Node 18+、Python 3 + wasmtime

```bash
rustup target add wasm32-unknown-unknown

# 1. 先生成可重定位引擎归档（相邻目录）
git clone https://github.com/yukumo-group/aquestalk-wasm.git
cd aquestalk-wasm
./scripts/build-reloc.sh --no-test

# 2. 构建 Yukw
cd ../Yukw
just wasm        # target/wasm32-unknown-unknown/release/aq_wasm.wasm
just kanji2koe   # target/wasm32-unknown-unknown/release/kanji2koe.wasm
```

没有 `just` 时：

```bash
cargo build -p aq-wasm --target wasm32-unknown-unknown --release --lib
cargo build -p kanji2koe --target wasm32-unknown-unknown --release --lib
```

如果两个仓库不是相邻目录：

```bash
AQTK_RELOC_DIR=/path/to/aquestalk-wasm/dist/reloc \
  cargo build -p aq-wasm --target wasm32-unknown-unknown --release --lib
```

host 侧检查：`just check` 或 `cargo check --workspace`。

## ABI

### `aq-wasm`

```c
int aq_synthesize(uint32_t text, uint32_t text_len, uint32_t param,
                  uint32_t phont_len, uint32_t out_ptr, uint32_t out_len);
void aq_free(uint32_t ptr);
```

`param` 是 40 字节、10 个 `i32`：

```
version, preset, speed, phont_data, base, volume, pitch, accent, lmd, fsc
```

- `version`：`1`（AquesTalk1）、`2`（AquesTalk2）、`10`（AquesTalk10）
- V1：`preset` 0–8，依次为 f1/f2/f3/m1/m2/r1/dvd/imd1/jgr
- V2：`preset` 0–16 选择内嵌 phont（`aq_yukkuri` 到 `ar_rm3`，然后是
  `aq_momo1`）。非空 `phont_data`（+ `phont_len`）覆盖 `preset`
- V10：`preset` 1–7 使用内置预设，0 使用 `base…fsc` 自定义
- 成功返回 0，并写出 WAV 指针/长度；失败返回负数，WAV 由 `aq_free` 释放

模块还导出 `malloc/free`、`aq_has_function`、`aq_error_message`、
`aq_static_engines`（返回链接的引擎数 5），以及五个引擎的直通 API。

### `kanji2koe`

```c
int aqk_load(uint32_t lookup, uint32_t lookup_len,
             uint32_t system, uint32_t system_len,
             uint32_t user, uint32_t user_len);
int aqk_convert(int32_t script, uint32_t text, uint32_t text_len,
                uint32_t out_ptr, uint32_t out_len);
void aqk_free(uint32_t ptr);
```

`script = 1` 表示日语。必须先 `aqk_load`，再 `aqk_convert`。`malloc/free`
同样导出，宿主用它放词典和输入文本。

## 示例

```bash
# 构建并复制模块
just wasm
cp target/wasm32-unknown-unknown/release/aq_wasm.wasm examples/yukw.wasm

# Python + wasmtime：每个 aq1 预设、aq2 phont、aq10 基本素片各生成一个 WAV
cd examples
./.venv/bin/python main.py          # 或 uv run main.py
```

## 测试

```bash
cargo test --workspace
```

- `aq-wasm` 和 `kanji2koe` 的 wasm 测试会真正构建 wasm32 模块，并检查
  **0 imports / 单 memory**。
- 缺少引擎归档时测试会跳过；可用 `AQTK_RELOC_DIR` 指定。
- `kanji2koe` 的转换测试还需要词典；可用 `AQK2K_DIC_DIR` 指定，否则跳过。

## 已知限制

- AquesTalk1、AquesTalk10、AqKanji2Koe 是评价版：ナ行/マ行会映射为ヌ。
- AquesTalk2 的预设 phont 编译在模块里。不在预设里的 phont 通过 `phont_data` 传入。
- `kanji2koe` 的词典在运行时加载，模块本身不再内嵌 9 MB 词典。
- `character`、`script` 仍是占位 crate。
- 旧的多 memory / embind 合并路径已经删除。`vendor/` 不再使用。
- 不提供 aqtk-shim 的 `AqShim_Init`/`.env` 许可证加载。wasm 参数块是
  `version` 到 `fsc` 这 10 个字段，不包含 `DevKey`/`UserKey`。当前链接的是
  评价版引擎，需要产品行为请使用带许可证的构建。

## 许可

AquesTalk 及其语音库、phont、词典是株式会社アクエスト（AQUEST Corp.）的商用
软件。本仓库的代码仅用于互操作性研究，不授予 AquesTalk 数据的使用、分发或
商业运行许可；需要产品行为请向 AQUEST 获取相应许可证。
