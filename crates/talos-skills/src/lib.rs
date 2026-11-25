//! Talos built-in skills.
//!
//! Responsible for the skills that ship inside the binary (DESIGN.md sections 2
//! and 6): each is a manifest under `skills/<id>/` at the workspace root,
//! embedded at build time, paired here with its compiled handler in
//! `src/<id>.rs`. How to add one: `docs/SKILL_AUTHORING.md`.

use std::borrow::Cow;

use talos_core::check::Report;
use talos_core::classifier::ModelFiles;
use talos_core::{Config, Engine, Error, Manifest, Registration, Skill};

include!(concat!(env!("OUT_DIR"), "/manifests.rs"));

// Shared by `timezones` and `preferences`: the place table and its zones.
mod places;
// Shared by `define`, `translate` and `teach`: wording pieces, lists, words.
mod dict;

/// Declares the built-in skills: each entry is `module::Handler`, where
/// `src/module.rs` defines the handler and `skills/module/manifest.toml` its
/// manifest. Declares the modules too.
macro_rules! skills {
    ($($module:ident :: $handler:ident),* $(,)?) => {
        $(mod $module;)*

        /// Every built-in handler, in catalogue order (docs/spec/README.md):
        /// the order `help` lists skills in.
        fn handlers() -> Vec<Box<dyn Skill>> {
            vec![$(Box::new($module::$handler)),*]
        }
    };
}

// The one shared line per skill: uncomment yours, in this order.
skills! {
    smalltalk::Smalltalk,
    identity::Identity,
    help::Help,
    clock::Clock,
    timezones::Timezones,
    calc::Calc,
    random::Random,
    fortune::Fortune,
    define::Define,
    teach::Teach,
    translate::Translate,
    preferences::Memory,
}

/// Whether the dictionary skills (`define`, `translate`) can read a
/// knowledge store in this build (the `knowledge` feature). Without it they
/// are installed all the same and answer that there is no dictionary.
pub const DICTIONARY: bool = cfg!(feature = "knowledge");

/// The embedded manifests as (directory name, TOML text), by directory name.
#[must_use]
pub fn manifest_sources() -> &'static [(&'static str, &'static str)] {
    MANIFESTS
}

/// The built-in skills, each manifest parsed and paired with its handler,
/// in catalogue order, with their examples' text (for the trainer).
///
/// # Errors
///
/// [`Error::Manifest`] if an embedded manifest is invalid or its id differs
/// from its directory; [`Error::Registry`] if a manifest has no handler or a
/// handler no manifest.
pub fn builtin() -> Result<Vec<Registration>, Error> {
    registrations(Manifest::parse)
}

/// As [`builtin`], keeping only the number and digest of the examples
/// ([`Manifest::parse_lean`]): all an engine needs of them.
///
/// # Errors
///
/// As [`builtin`].
pub fn builtin_lean() -> Result<Vec<Registration>, Error> {
    registrations(Manifest::parse_lean)
}

fn registrations(
    parse: fn(&str, &str) -> Result<Manifest, Error>,
) -> Result<Vec<Registration>, Error> {
    let mut skills = Vec::with_capacity(MANIFESTS.len());
    let mut sources: Vec<&(&str, &str)> = MANIFESTS.iter().collect();
    for handler in handlers() {
        let position = sources
            .iter()
            .position(|(dir, _)| *dir == handler.id())
            .ok_or_else(|| {
                Error::Registry(format!("handler `{}` has no manifest", handler.id()))
            })?;
        let (dir, text) = sources.remove(position);
        let manifest = parse(dir, text)?;
        if manifest.id != *dir {
            return Err(Error::Manifest {
                skill: (*dir).to_owned(),
                message: format!("id `{}` differs from its directory", manifest.id),
            });
        }
        skills.push((manifest, handler));
    }
    if let Some((dir, _)) = sources.first() {
        return Err(Error::Registry(format!("skill `{dir}` has no handler")));
    }
    Ok(skills)
}

/// The classifier models embedded at build time (`models/`, written by
/// `cargo run --release -p talos-train`). Read in place: the binary's own
/// pages, touched only where a turn reads them.
#[must_use]
pub fn models() -> ModelFiles {
    ModelFiles {
        intent: INTENT_MODEL.map(Cow::Borrowed),
        language: LANGUAGE_MODEL.map(Cow::Borrowed),
    }
}

/// An engine with every built-in skill and the embedded classifiers (a
/// model that does not match the manifests is refused: rules only).
///
/// # Errors
///
/// As [`builtin`] and [`Engine::new`].
pub fn engine(config: Config) -> Result<Engine, Error> {
    Ok(Engine::new(config, builtin_lean()?)?.with_models(models()))
}

/// An engine with every built-in skill and no classifier: rules only, as
/// in v0.1. For comparisons (`talos eval`).
///
/// # Errors
///
/// As [`builtin`] and [`Engine::new`].
pub fn rules_only_engine(config: Config) -> Result<Engine, Error> {
    Engine::new(config, builtin_lean()?)
}

/// Validates every built-in manifest against its handler: what `talos
/// skills check` prints.
#[must_use]
pub fn check() -> Report {
    talos_core::check::check(MANIFESTS, &handlers())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_embedded_manifest_loads_with_its_handler() {
        let skills = builtin().unwrap();
        assert_eq!(skills.len(), MANIFESTS.len());
        assert_eq!(
            skills[0].0.id, "smalltalk",
            "catalogue order starts with smalltalk"
        );
        assert!(engine(Config::default()).is_ok());
        // The lean form is the same manifests without the examples' text.
        let lean = builtin_lean().unwrap();
        for ((full, _), (lean, _)) in skills.iter().zip(&lean) {
            assert_eq!(full.intents.len(), lean.intents.len());
            for (a, b) in full.intents.iter().zip(&lean.intents) {
                assert_eq!(a.examples.es.digest(), b.examples.es.digest());
                assert_eq!(a.examples.en.len(), b.examples.en.len());
                assert_eq!(b.examples.en.texts(), [] as [String; 0]);
                assert_eq!(a.patterns, b.patterns);
            }
            assert_eq!(full.replies, lean.replies);
        }
    }

    #[test]
    fn the_embedded_models_match_the_manifests() {
        // Fails when a manifest's examples or the out-of-scope corpus changed
        // and the models were not retrained: `cargo run --release -p talos-train`.
        let engine = engine(Config::default()).unwrap();
        let classifiers = engine.classifiers();
        let intent = classifiers.intent_model().unwrap();
        let language = classifiers.language_model().unwrap();
        intent.verify().unwrap();
        language.verify().unwrap();
        // DESIGN.md section 8: the model file is at most 2 MB.
        assert!(intent.file_len() + language.file_len() <= 2_000_000);
    }

    #[test]
    fn the_built_in_skills_pass_the_check() {
        let report = check();
        let findings: Vec<String> = report.findings.iter().map(ToString::to_string).collect();
        assert!(findings.is_empty(), "{findings:#?}");
    }
}
