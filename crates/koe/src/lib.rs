//! Contract for text-to-reading converters.
//! A `*2koe` crate implements [`ToKoe`]. Callers depend on this crate
//! and can swap one dictionary for another.

/// A converter rejected `text` or failed while producing a reading.
/// `code` is the converter's own negative status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error {
    pub code: i32,
}

impl Error {
    pub const fn new(code: i32) -> Self {
        Self { code }
    }
}

/// Turn source text into a reading.
pub trait ToKoe {
    fn to_koe(&mut self, text: &str) -> Result<String, Error>;
}
