/* aq_wasm.h — ABI for aq_wasm.wasm
 *
 * Companion header for the single-memory Yukw module produced by the
 * `aq-wasm` crate.  The module has 0 imports and exports one linear memory
 * named `memory`.  Every pointer argument below is an offset into that
 * memory (i32 / uint32_t from a JS or wasmtime host).
 *
 * High-level synthesis goes through aq_synthesize / aq_free.  The prefixed
 * engine entry points (aq1_*, aq2_*, aq10_*, aqk2k_*, aqusr_*) are also
 * exported for hosts that talk to one engine directly.
 *
 * Parameter, preset, and error numbering match aqtk-shim's Aqsh.h; there is
 * no AqShim_Init and no DevKey/UserKey field in the wasm param block.
 */

#ifndef YUKW_AQ_WASM_H
#define YUKW_AQ_WASM_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ------------------------------------------------------------------ */
/* Versions and presets (same discriminants as Aqsh.h)                */
/* ------------------------------------------------------------------ */

typedef enum AqVersion {
    AQ_VER_1 = 1,
    AQ_VER_2 = 2,
    AQ_VER_10 = 10
} AqVersion;

typedef enum Aq1Preset {
    AQ1_F1 = 0,
    AQ1_F2,
    AQ1_F3,
    AQ1_M1,
    AQ1_M2,
    AQ1_R1,
    AQ1_DVD,
    AQ1_IMD1,
    AQ1_JGR,
    AQ1_PRESET_COUNT
} Aq1Preset;

/* Inlined AquesTalk2 phonts. 0 is aq_yukkuri; 16 is aq_momo1.
 * A non-zero phont_data (+ phont_len on aq_synthesize) overrides preset. */
typedef enum Aq2Preset {
    AQ2_AQ_YUKKURI = 0,
    AQ2_AQ_DEFO1,
    AQ2_AQ_F1C,
    AQ2_AQ_F3A,
    AQ2_AQ_HUSKEY,
    AQ2_AQ_M4B,
    AQ2_AQ_MF1,
    AQ2_AQ_RB2,
    AQ2_AQ_RB3,
    AQ2_AQ_RM,
    AQ2_AQ_ROBO,
    AQ2_AQ_TETO1,
    AQ2_AR_F4,
    AQ2_AR_M5,
    AQ2_AR_MF2,
    AQ2_AR_RM3,
    AQ2_AQ_MOMO1,
    AQ2_PRESET_COUNT
} Aq2Preset;

/* 0 uses base/speed/volume/pitch/accent/lmd/fsc; 1-7 are named presets. */
typedef enum Aq10Preset {
    AQ10_CUSTOM = 0,
    AQ10_F1,
    AQ10_F2,
    AQ10_F3,
    AQ10_M1,
    AQ10_M2,
    AQ10_R1,
    AQ10_R2,
    AQ10_PRESET_COUNT
} Aq10Preset;

/* Host-side error codes returned by aq_synthesize before an engine runs. */
enum {
    AQ_ERR_ARG = -1,
    AQ_ERR_VERSION = -2,
    AQ_ERR_OPEN = -3
};

/* ------------------------------------------------------------------ */
/* High-level synthesis (preferred host surface)                      */
/* ------------------------------------------------------------------ */

/* 40-byte little-endian param block written into linear memory.
 * phont_data is a wasm offset (0 = none).  There is no phont path and no
 * license key field. */
typedef struct AqSynthParam {
    int32_t version;     /* AqVersion: 1, 2, or 10 */
    int32_t preset;      /* Aq1Preset / Aq2Preset / Aq10Preset */
    int32_t speed;       /* typically 50-300; 100 is default */
    uint32_t phont_data; /* V2: offset of phont bytes, or 0 */
    int32_t base;        /* V10 custom: 基本素片 F1E/F2E/M1E (0/1/2) */
    int32_t volume;      /* V10: 0-300, default 100 */
    int32_t pitch;       /* V10: 20-200 */
    int32_t accent;      /* V10: 0-200 */
    int32_t lmd;         /* V10: 0-200, default 100 */
    int32_t fsc;         /* V10: 50-200 (sample-rate scale), default 100 */
} AqSynthParam;

/* Synthesize UTF-8 text[text_len] with the param at `param`.
 * phont_len is the byte length of phont_data when that pointer is set.
 * On success returns 0 and stores the WAV offset at *out_ptr and its length
 * at *out_len.  On failure returns a negative code and clears both outs.
 * Release a successful WAV with aq_free. */
int32_t aq_synthesize(uint32_t text, uint32_t text_len, uint32_t param,
                      uint32_t phont_len, uint32_t out_ptr, uint32_t out_len);

void aq_free(uint32_t pointer);

/* No-op retained for API parity with the previous embind host. */
void aq_set_clock(double ms);

/* NUL-terminated AQ1_/AQ2_/AQ10_ error token in module memory, or 0. */
uint32_t aq_error_message(int32_t version, int32_t code);

/* 1 when the linked engine accepts version/preset for name
 * ("synthe", "verifyLicense", "issueBalanceToken"). */
int32_t aq_has_function(int32_t version, int32_t preset, uint32_t name,
                        uint32_t name_len);

/* Number of engines linked into this module (currently 5). */
uint32_t aq_static_engines(void);

/* Host heap for staging text, params, phonts, and dictionary bytes. */
void *malloc(uint32_t size);
void *calloc(uint32_t count, uint32_t size);
void *realloc(void *pointer, uint32_t size);
void free(void *pointer);

/* ------------------------------------------------------------------ */
/* Direct engine exports (prefixed relocatable ABIs)                  */
/* ------------------------------------------------------------------ */

/* AquesTalk1 */
void *aq1_alloc(uint32_t size);
void aq1_free(void *pointer);
int32_t aq1_select_voice(const char *name);
int32_t aq1_voice_count(void);
const char *aq1_voice_name(int32_t index);
unsigned char *aq1_synthe_utf8(const char *utf8, int32_t speed, int32_t *size);
void aq1_free_wave(unsigned char *wave);

/* AquesTalk2 */
void *aq2_alloc(uint32_t size);
void aq2_free(void *pointer);
unsigned char *aq2_synthe(const char *koe, int32_t speed, int32_t *size,
                          void *phont);
unsigned char *aq2_synthe_euc(const char *koe, int32_t speed, int32_t *size,
                              void *phont);
unsigned char *aq2_synthe_utf8(const char *koe, int32_t speed, int32_t *size,
                               void *phont);
unsigned char *aq2_synthe_utf16(const uint16_t *koe, int32_t speed,
                                int32_t *size, void *phont);
unsigned char *aq2_synthe_roman(const char *koe, int32_t speed, int32_t *size,
                                void *phont);
void aq2_free_wave(unsigned char *wave);
int32_t aq2_last_error(void);
int32_t aq2_sample_rate(void); /* always 8000 */
unsigned char *aq2_image_base(void);
const unsigned char *aq2_builtin_phont(int32_t preset, uint32_t *size);

/* AquesTalk10 */
void aq10_init(void);
uint32_t aq10_alloc(uint32_t size);
void aq10_free(void *pointer);

typedef struct Aq10Voice {
    int32_t bas, spd, vol, pit, acc, lmd, fsc;
} Aq10Voice;

unsigned char *aq10_synthe_utf8(const Aq10Voice *voice, const char *koe,
                                int32_t *size);
unsigned char *aq10_synthe_sjis(const Aq10Voice *voice, const char *koe,
                                int32_t *size);
unsigned char *aq10_synthe_utf16(const Aq10Voice *voice, const uint16_t *koe,
                                 int32_t *size);
unsigned char *aq10_synthe_utf8_default(const char *koe, int32_t bas,
                                        int32_t spd, int32_t vol, int32_t pit,
                                        int32_t acc, int32_t lmd, int32_t fsc,
                                        int32_t *size);
void aq10_free_wave(unsigned char *wave);
int32_t aq10_set_dev_key(const char *key);
int32_t aq10_set_usr_key(const char *key);

/* AqKanji2Koe — host supplies lookup.bin / aqdic.bin / optional user dic. */
void aqk2k_init(void);
void *aqk2k_alloc(uint32_t size);
void aqk2k_free(void *pointer);
int32_t aqk2k_set_lookup(const void *data, uint32_t size);
void *aqk2k_create(const void *sys_dic, const void *user_dic, int32_t *err);
void aqk2k_release(void *handle);
int32_t aqk2k_convert(void *handle, const char *kanji, char *koe,
                      int32_t buf_size);
int32_t aqk2k_conv_roman(void *handle, const char *kanji, char *koe,
                         int32_t buf_size);
int32_t aqk2k_set_devkey(const char *key);

/* AqUsrDic — CSV <-> aq_user.dic; call aqusr_set_system_dic first. */
void aqusr_init(void);
void *aqusr_alloc(uint32_t size);
void aqusr_free(void *pointer);
int32_t aqusr_set_system_dic(const void *aqdic, uint32_t len);
int32_t aqusr_import(const char *csv, uint32_t csv_len, void *out,
                     uint32_t out_cap, uint32_t *out_len);
int32_t aqusr_export(const void *dic, uint32_t dic_len, char *out,
                     uint32_t out_cap, uint32_t *out_len);
int32_t aqusr_check(const char *surface, const char *yomi, int32_t pos);
const char *aqusr_error(void);

#ifdef __cplusplus
}
#endif

#endif /* YUKW_AQ_WASM_H */
