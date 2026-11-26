//! The network as a handler sees it: one way to read a page, given by the
//! front end.
//!
//! The engine opens no connection and links no network code (DESIGN.md
//! section 2). A front end that is allowed the network gives the engine a
//! [`Fetch`] ([`crate::Engine::use_network`]); a handler of a skill that
//! declares the `network` capability reaches it through
//! [`crate::Context::network`], and says so when there is none. Tests give
//! one that answers from recorded pages.

use std::fmt;

/// Why a page could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchError {
    /// The server answered that there is no such page (404).
    NotFound,
    /// Anything else: no connection, a timeout, a refusal, a server error.
    /// The text is for a log, not for the user.
    Failed(String),
}

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("not found"),
            Self::Failed(why) => f.write_str(why),
        }
    }
}

/// Reads a page. An implementation bounds the time a request may take and
/// the size of what it returns: a turn must never hang on it.
pub trait Fetch: Send + Sync + fmt::Debug {
    /// The body of `url` as text.
    ///
    /// # Errors
    ///
    /// [`FetchError`] when the page does not exist or could not be read.
    fn get(&self, url: &str) -> Result<String, FetchError>;
}

/// `text` as one path segment or query value of a URL: everything but
/// unreserved characters percent-encoded, as UTF-8.
#[must_use]
pub fn encode(text: &str) -> String {
    use fmt::Write as _;
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.~".contains(&byte) {
            out.push(char::from(byte));
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::encode;

    #[test]
    fn a_title_is_encoded_as_utf8() {
        assert_eq!(encode("Ramón y Cajal"), "Ram%C3%B3n%20y%20Cajal");
        assert_eq!(encode("AC/DC?"), "AC%2FDC%3F");
    }
}
