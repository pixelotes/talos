//! `cargo run --release -p talos-train`: retrains the intent and language
//! models from the built-in manifests and `reflector/out-of-scope.toml`,
//! and writes them to `models/` (docs/MODEL.md).
//!
//! ```text
//! talos-train                 train and write models/intent.model, models/language.model
//! talos-train --check         train and fail if the files differ from what training gives
//! talos-train --cv K          K-fold cross-validation on the training corpus; writes nothing
//! talos-train --cv K --misses  the same, listing the training phrases it got wrong
//! talos-train --out DIR       write to DIR instead of models/
//! ```
//!
//! Experiments may override the shipped parameters with `--epochs N`,
//! `--l2 X`, `--rate X`, `--prune X`, `--threshold X`, `--margin X`,
//! `--min-n N`, `--max-n N`, `--no-words`, `--no-pairs` (intent model only).

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "a command-line tool"
)]

use std::path::PathBuf;
use std::process::ExitCode;

use talos_core::Lang;
use talos_core::classifier::{self, Model, ModelData};
use talos_train::{
    Params, class_slots, corpus, cross_validate, intent_classes, train_intent, train_language,
};

struct Options {
    check: bool,
    cv: Option<usize>,
    misses: bool,
    out: PathBuf,
    intent: Params,
}

fn value<T: std::str::FromStr>(
    args: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<T, String> {
    args.next()
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| format!("`{flag}` needs a value"))
}

fn options() -> Result<Options, String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut options = Options {
        check: false,
        cv: None,
        misses: false,
        out: root.join("models"),
        intent: Params::intent(),
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let p = &mut options.intent;
        match arg.as_str() {
            "--check" => options.check = true,
            "--cv" => options.cv = Some(value(&mut args, &arg)?),
            "--misses" => options.misses = true,
            "--out" => options.out = PathBuf::from(value::<String>(&mut args, &arg)?),
            "--epochs" => p.epochs = value(&mut args, &arg)?,
            "--balance" => p.balance = value(&mut args, &arg)?,
            "--rare" => p.rare = value(&mut args, &arg)?,
            "--l2" => p.l2 = value(&mut args, &arg)?,
            "--rate" => p.rate = value(&mut args, &arg)?,
            "--prune" => p.prune = value(&mut args, &arg)?,
            "--threshold" => p.threshold = value(&mut args, &arg)?,
            "--margin" => p.margin = value(&mut args, &arg)?,
            "--short-words" => p.short_words = value(&mut args, &arg)?,
            "--short-threshold" => p.short_threshold = value(&mut args, &arg)?,
            "--min-n" => p.features.min_n = value(&mut args, &arg)?,
            "--max-n" => p.features.max_n = value(&mut args, &arg)?,
            "--no-words" => p.features.words = false,
            "--no-pairs" => p.features.word_pairs = false,
            other => return Err(format!("unknown argument `{other}`")),
        }
    }
    Ok(options)
}

#[allow(clippy::cast_precision_loss, reason = "counts are small")]
fn pct(part: usize, whole: usize) -> f64 {
    100.0 * part as f64 / whole.max(1) as f64
}

fn describe(name: &str, data: &ModelData, bytes: &[u8]) -> String {
    let weights: usize = data.vocabulary.iter().map(|e| e.weights.len()).sum();
    format!(
        "{name}: {} classes, {} features, {weights} weights, {} bytes, threshold {}, margin {}",
        data.classes.len(),
        data.vocabulary.len(),
        bytes.len(),
        data.threshold,
        data.margin
    )
}

fn run() -> Result<bool, String> {
    let options = options()?;
    let skills = talos_skills::builtin().map_err(|e| e.to_string())?;
    let manifests: Vec<_> = skills.into_iter().map(|(m, _)| m).collect();
    let out_of_scope = classifier::parse_out_of_scope(classifier::OUT_OF_SCOPE_SOURCE)
        .map_err(|e| e.to_string())?;
    let hash = classifier::corpus_hash(&manifests, &out_of_scope);
    let corpus = corpus(&manifests, &out_of_scope);
    for lang in Lang::ALL {
        let all = corpus.iter().filter(|e| e.lang == lang).count();
        let oos = corpus
            .iter()
            .filter(|e| e.lang == lang && e.class == classifier::OUT_OF_SCOPE)
            .count();
        println!("[{lang}] {} intent examples, {oos} out-of-scope", all - oos);
    }

    if let Some(k) = options.cv {
        let (result, misses) = cross_validate(&corpus, k, options.intent, Params::language());
        if options.misses {
            for (text, class, got) in &misses {
                println!("{class:<24} {:<24} {text}", got.as_deref().unwrap_or("-"));
            }
        }
        for lang in Lang::ALL {
            let r = result.get(lang);
            println!(
                "[{lang}] {k}-fold: intents {:.1}% right, {:.1}% confused, {:.1}% rejected; \
                 out-of-scope accepted {:.1}%; language confident and right {:.1}%, wrong {:.1}%",
                pct(r.correct, r.intents),
                pct(r.confused, r.intents),
                pct(r.intents - r.correct - r.confused, r.intents),
                pct(r.false_accepts, r.out_of_scope),
                pct(r.lang_right, r.lang_rows),
                pct(r.lang_wrong, r.lang_rows),
            );
        }
        return Ok(true);
    }

    let mut same = true;
    let mut intent = train_intent(&corpus, hash, options.intent);
    intent.slots = class_slots(&manifests, &corpus, &intent_classes(&corpus));
    for (class, slots) in intent.classes.iter().zip(&intent.slots) {
        if !slots.is_empty() {
            let list: Vec<String> = slots.iter().map(|(k, v)| format!("{k}={v}")).collect();
            println!("{class}: a guess carries {}", list.join(" "));
        }
    }
    for (name, data) in [
        ("intent", intent),
        (
            "language",
            train_language(&corpus, hash, Params::language()),
        ),
    ] {
        let bytes = data.encode().map_err(|e| e.to_string())?;
        let model = Model::open(bytes.clone()).map_err(|e| e.to_string())?;
        model.verify().map_err(|e| e.to_string())?;
        println!("{}", describe(name, &data, &bytes));
        let path = options.out.join(format!("{name}.model"));
        if options.check {
            let on_disk = std::fs::read(&path).unwrap_or_default();
            if on_disk != bytes {
                eprintln!(
                    "{} is not what training gives; run `cargo run --release -p talos-train`",
                    path.display()
                );
                same = false;
            }
        } else {
            std::fs::create_dir_all(&options.out).map_err(|e| e.to_string())?;
            std::fs::write(&path, &bytes).map_err(|e| format!("{}: {e}", path.display()))?;
            println!("wrote {}", path.display());
        }
    }
    println!("corpus {hash:016x}");
    Ok(same)
}

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("talos-train: {error}");
            ExitCode::from(2)
        }
    }
}
