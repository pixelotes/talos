//! Talos built-in skills.
//!
//! Responsible for the skills that ship inside the binary (DESIGN.md sections 2
//! and 6): each is a manifest under `skills/<id>/` at the workspace root,
//! embedded at build time, paired here with its compiled handler.

mod smalltalk;

use talos_core::{Config, Engine, Error, Manifest, Registration, Skill};

pub use smalltalk::Smalltalk;

include!(concat!(env!("OUT_DIR"), "/manifests.rs"));

/// Every built-in handler. Adding a skill is one line here plus its
/// manifest directory.
fn handlers() -> Vec<Box<dyn Skill>> {
    vec![Box::new(Smalltalk)]
}

/// The embedded manifests as (directory name, TOML text).
#[must_use]
pub fn manifest_sources() -> &'static [(&'static str, &'static str)] {
    MANIFESTS
}

/// The built-in skills, each manifest parsed and paired with its handler.
///
/// # Errors
///
/// [`Error::Manifest`] if an embedded manifest is invalid or its id differs
/// from its directory; [`Error::Registry`] if a manifest has no handler or a
/// handler no manifest.
pub fn builtin() -> Result<Vec<Registration>, Error> {
    let mut handlers = handlers();
    let mut skills = Vec::with_capacity(MANIFESTS.len());
    for (dir, text) in MANIFESTS {
        let manifest = Manifest::parse(dir, text)?;
        if manifest.id != *dir {
            return Err(Error::Manifest {
                skill: (*dir).to_owned(),
                message: format!("id `{}` differs from its directory", manifest.id),
            });
        }
        let position = handlers
            .iter()
            .position(|h| h.id() == manifest.id)
            .ok_or_else(|| Error::Registry(format!("skill `{}` has no handler", manifest.id)))?;
        skills.push((manifest, handlers.swap_remove(position)));
    }
    if let Some(orphan) = handlers.first() {
        return Err(Error::Registry(format!(
            "handler `{}` has no manifest",
            orphan.id()
        )));
    }
    Ok(skills)
}

/// An engine with every built-in skill.
///
/// # Errors
///
/// As [`builtin`] and [`Engine::new`].
pub fn engine(config: Config) -> Result<Engine, Error> {
    Engine::new(config, builtin()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_embedded_manifest_loads_with_its_handler() {
        let skills = builtin().unwrap();
        let ids: Vec<&str> = skills.iter().map(|(m, _)| m.id.as_str()).collect();
        assert_eq!(ids, ["smalltalk"]);
        assert!(engine(Config::default()).is_ok());
    }
}
