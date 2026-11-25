//! Errors raised while building the engine or rendering a reply. A turn never
//! returns an error: whatever happens inside it becomes an honest outcome.

use std::fmt;

/// Something the engine could not accept or produce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A manifest is malformed or incomplete.
    Manifest {
        /// The skill id, or the source name when the id is unknown.
        skill: String,
        /// What is wrong.
        message: String,
    },
    /// The set of skills is inconsistent (duplicates, missing handlers).
    Registry(String),
    /// A reply could not be rendered.
    Render(String),
    /// A reflector script is malformed (docs/spec/reflector.md).
    Script {
        /// The script, e.g. `reflector/es.toml`.
        script: String,
        /// What is wrong.
        message: String,
    },
}

impl Error {
    pub(crate) fn manifest(skill: &str, message: impl Into<String>) -> Self {
        Error::Manifest {
            skill: skill.to_owned(),
            message: message.into(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Manifest { skill, message } => write!(f, "manifest `{skill}`: {message}"),
            Error::Registry(message) => write!(f, "skill registry: {message}"),
            Error::Render(message) => write!(f, "render: {message}"),
            Error::Script { script, message } => write!(f, "script `{script}`: {message}"),
        }
    }
}

impl std::error::Error for Error {}
