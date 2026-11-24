//! The Talos knowledge store.
//!
//! Responsible for the schema, queries, reasoning and knowledge packs
//! (DESIGN.md sections 2 and 5), backed by SQLite.

/// Name of this crate, as reported in diagnostics.
#[must_use]
pub const fn crate_name() -> &'static str {
    "talos-knowledge"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_its_name() {
        assert_eq!(crate_name(), "talos-knowledge");
    }
}
