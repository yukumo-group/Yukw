//! Engine error names from `aqtk-shim/src/aq_error.cpp`.
//! A negative code is the same failure as its positive form.
//! Unknown codes return `None`.

use crate::param::AqVersion;

struct ErrorRow {
    /// 0 applies to every engine. Otherwise, an [`AqVersion`] discriminant.
    version: i32,
    code: i32,
    name: &'static str,
}

const ROWS: &[ErrorRow] = &[
    ErrorRow {
        version: 0,
        code: 100,
        name: "OTHER_ERROR",
    },
    ErrorRow {
        version: 0,
        code: 101,
        name: "INSUFFICIENT_MEMORY",
    },
    ErrorRow {
        version: 0,
        code: 105,
        name: "UNDEFINED_READING_SYMBOL",
    },
    ErrorRow {
        version: 0,
        code: 106,
        name: "INVALID_TAG",
    },
    ErrorRow {
        version: 0,
        code: 107,
        name: "TAG_LENGTH_EXCEEDED",
    },
    ErrorRow {
        version: 0,
        code: 108,
        name: "INVALID_TAG_VALUE",
    },
    ErrorRow {
        version: 1,
        code: 200,
        name: "PHONETIC_TOO_LONG",
    },
    ErrorRow {
        version: 1,
        code: 201,
        name: "TOO_MANY_READING_SYMBOLS",
    },
    ErrorRow {
        version: 1,
        code: 202,
        name: "PHONETIC_TOO_LONG",
    },
    ErrorRow {
        version: 1,
        code: 203,
        name: "INSUFFICIENT_HEAP_MEMORY",
    },
    ErrorRow {
        version: 1,
        code: 204,
        name: "PHONETIC_TOO_LONG",
    },
    ErrorRow {
        version: 2,
        code: 102,
        name: "UNDEFINED_READING_SYMBOL",
    },
    ErrorRow {
        version: 2,
        code: 103,
        name: "NEGATIVE_PROSODY_DURATION",
    },
    ErrorRow {
        version: 2,
        code: 104,
        name: "UNDEFINED_DELIMITER",
    },
    ErrorRow {
        version: 2,
        code: 111,
        name: "NO_SPEECH_DATA",
    },
    ErrorRow {
        version: 2,
        code: 200,
        name: "PHONETIC_TOO_LONG",
    },
    ErrorRow {
        version: 2,
        code: 201,
        name: "TOO_MANY_READING_SYMBOLS",
    },
    ErrorRow {
        version: 2,
        code: 202,
        name: "PHONETIC_BUFFER_OVERFLOW",
    },
    ErrorRow {
        version: 2,
        code: 203,
        name: "INSUFFICIENT_HEAP_MEMORY",
    },
    ErrorRow {
        version: 2,
        code: 204,
        name: "PHONETIC_BUFFER_OVERFLOW",
    },
    ErrorRow {
        version: 10,
        code: 103,
        name: "INVALID_PHONETIC_SPEC",
    },
    ErrorRow {
        version: 10,
        code: 104,
        name: "NO_VALID_READING",
    },
    ErrorRow {
        version: 10,
        code: 120,
        name: "PHONETIC_TOO_LONG",
    },
    ErrorRow {
        version: 10,
        code: 121,
        name: "TOO_MANY_READING_SYMBOLS",
    },
    ErrorRow {
        version: 10,
        code: 122,
        name: "PHONETIC_BUFFER_OVERFLOW",
    },
];

fn error_name(version: AqVersion, code: i32) -> Option<&'static str> {
    if version == AqVersion::V2 && (1000..=1008).contains(&code) {
        return Some("INVALID_PHONT");
    }
    let version = version as i32;
    ROWS.iter()
        .find(|row| row.code == code && (row.version == 0 || row.version == version))
        .map(|row| row.name)
}

fn join(version: AqVersion, name: &'static str) -> &'static str {
    match (version, name) {
        (AqVersion::V1, "OTHER_ERROR") => "AQ1_OTHER_ERROR",
        (AqVersion::V1, "INSUFFICIENT_MEMORY") => "AQ1_INSUFFICIENT_MEMORY",
        (AqVersion::V1, "UNDEFINED_READING_SYMBOL") => "AQ1_UNDEFINED_READING_SYMBOL",
        (AqVersion::V1, "INVALID_TAG") => "AQ1_INVALID_TAG",
        (AqVersion::V1, "TAG_LENGTH_EXCEEDED") => "AQ1_TAG_LENGTH_EXCEEDED",
        (AqVersion::V1, "INVALID_TAG_VALUE") => "AQ1_INVALID_TAG_VALUE",
        (AqVersion::V1, "PHONETIC_TOO_LONG") => "AQ1_PHONETIC_TOO_LONG",
        (AqVersion::V1, "TOO_MANY_READING_SYMBOLS") => "AQ1_TOO_MANY_READING_SYMBOLS",
        (AqVersion::V1, "INSUFFICIENT_HEAP_MEMORY") => "AQ1_INSUFFICIENT_HEAP_MEMORY",
        (AqVersion::V2, "OTHER_ERROR") => "AQ2_OTHER_ERROR",
        (AqVersion::V2, "INSUFFICIENT_MEMORY") => "AQ2_INSUFFICIENT_MEMORY",
        (AqVersion::V2, "UNDEFINED_READING_SYMBOL") => "AQ2_UNDEFINED_READING_SYMBOL",
        (AqVersion::V2, "INVALID_TAG") => "AQ2_INVALID_TAG",
        (AqVersion::V2, "TAG_LENGTH_EXCEEDED") => "AQ2_TAG_LENGTH_EXCEEDED",
        (AqVersion::V2, "INVALID_TAG_VALUE") => "AQ2_INVALID_TAG_VALUE",
        (AqVersion::V2, "NEGATIVE_PROSODY_DURATION") => "AQ2_NEGATIVE_PROSODY_DURATION",
        (AqVersion::V2, "UNDEFINED_DELIMITER") => "AQ2_UNDEFINED_DELIMITER",
        (AqVersion::V2, "NO_SPEECH_DATA") => "AQ2_NO_SPEECH_DATA",
        (AqVersion::V2, "PHONETIC_TOO_LONG") => "AQ2_PHONETIC_TOO_LONG",
        (AqVersion::V2, "TOO_MANY_READING_SYMBOLS") => "AQ2_TOO_MANY_READING_SYMBOLS",
        (AqVersion::V2, "PHONETIC_BUFFER_OVERFLOW") => "AQ2_PHONETIC_BUFFER_OVERFLOW",
        (AqVersion::V2, "INSUFFICIENT_HEAP_MEMORY") => "AQ2_INSUFFICIENT_HEAP_MEMORY",
        (AqVersion::V2, "INVALID_PHONT") => "AQ2_INVALID_PHONT",
        (AqVersion::V10, "OTHER_ERROR") => "AQ10_OTHER_ERROR",
        (AqVersion::V10, "INSUFFICIENT_MEMORY") => "AQ10_INSUFFICIENT_MEMORY",
        (AqVersion::V10, "UNDEFINED_READING_SYMBOL") => "AQ10_UNDEFINED_READING_SYMBOL",
        (AqVersion::V10, "INVALID_TAG") => "AQ10_INVALID_TAG",
        (AqVersion::V10, "TAG_LENGTH_EXCEEDED") => "AQ10_TAG_LENGTH_EXCEEDED",
        (AqVersion::V10, "INVALID_TAG_VALUE") => "AQ10_INVALID_TAG_VALUE",
        (AqVersion::V10, "INVALID_PHONETIC_SPEC") => "AQ10_INVALID_PHONETIC_SPEC",
        (AqVersion::V10, "NO_VALID_READING") => "AQ10_NO_VALID_READING",
        (AqVersion::V10, "PHONETIC_TOO_LONG") => "AQ10_PHONETIC_TOO_LONG",
        (AqVersion::V10, "TOO_MANY_READING_SYMBOLS") => "AQ10_TOO_MANY_READING_SYMBOLS",
        (AqVersion::V10, "PHONETIC_BUFFER_OVERFLOW") => "AQ10_PHONETIC_BUFFER_OVERFLOW",
        _ => "AQ_UNKNOWN",
    }
}

/// `AQ1_<NAME>`, `AQ2_<NAME>`, or `AQ10_<NAME>`.
pub fn error_message(version: AqVersion, code: i32) -> Option<&'static str> {
    let code = if code < 0 { -code } else { code };
    let name = error_name(version, code)?;
    let text = join(version, name);
    if text == "AQ_UNKNOWN" {
        None
    } else {
        Some(text)
    }
}
