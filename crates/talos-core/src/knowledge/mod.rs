//! The knowledge store (docs/KNOWLEDGE.md; ADRs 0005 and 0006), behind the
//! `knowledge` feature: SQLite through `rusqlite`, synchronous, no async.
//!
//! - [`schema`]: the tables and their forward-only migrations.
//! - [`UserDb`]: `user.db`, where preferences and corrections are kept (and,
//!   later, what the user teaches); imports `user.toml` once.
//! - [`Knowledge`]: `user.db` and the attached packs, read-only, queried as
//!   one: a word to its senses through its inflected forms, glosses and
//!   words per language, sense choice, stock phrases, the word-pair
//!   lexicon. Handlers reach it through [`crate::Context::knowledge`].
//! - [`pack`]: what a pack says about itself, and its checks.
//! - [`build`]: a pack from its hand-written source (`knowledge/seed.toml`).

pub mod build;
mod inflect;
pub mod pack;
mod query;
pub mod schema;
mod senses;
mod store;
mod userdb;

pub use inflect::{key, plural};
pub use pack::{Checked, Counts, PackInfo, Refusal, Source};
pub use query::{
    ConceptId, Entry, Gloss, Lookup, PhraseText, Section, Sense, StockPhrase, Word, WordPairs,
};
pub use senses::{Cues, Label, Relation, SenseChoice, SenseOption, Step};
pub use store::{Knowledge, PACK_CACHE_KIB, pack_files};
pub use userdb::{FILE_NAME as USER_DB, MIGRATED_NAME, UserDb};
