//! Golden transcripts: a plain-text format mirroring the exchange format of
//! docs/spec/README.md, a parser, a runner and a way to regenerate expected
//! output deliberately. The format is documented in docs/DEVELOPMENT.md.
//!
//! ```text
//! # A comment.
//! Given: now = 2027-03-14 18:42 Europe/Madrid
//! Given: draw = 4
//!
//! [es] U: tira un dado
//!      T: Ha salido un 4.                          → answered · random.dice · count=1 sides=6
//! ```
//!
//! Turns with no blank line between them are one conversation; a blank line
//! starts a new one. `Given:` settings last until changed, except `draw`,
//! which applies to the next turn only. What is stored about the user
//! (`Given: stored …`, `Given: phrase …`) is set afresh at the start of
//! every conversation; within one, the turns change it. The reply and the
//! outcome line are compared byte for byte, and the `[es]`/`[en]` tag with
//! the turn's language.

use std::fmt::Write as _;
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};

use crate::civil::Date;
use crate::random::Draw;
use crate::user::{Correction, UserData};
use crate::{Config, ConvId, Engine, Error, Lang, Place, Preferences, Timestamp, Turn, Units};

/// The column where a same-line outcome arrow starts.
const ARROW_COLUMN: usize = 50;

/// Settings a `Given:` line can change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// `now = 2027-03-14 18:42 Europe/Madrid`; the default is that one, the
    /// spec's (docs/spec/conversation.md).
    pub now: Timestamp,
    /// `seed = 7`; default 0.
    pub seed: u64,
    /// `default language = en`; default `es`.
    pub default_lang: Lang,
    /// `hint = en` or `hint = none`: the caller's language hint.
    pub hint: Option<Lang>,
    /// `place = Madrid` or `place = none`: the caller's place.
    pub place: Option<Place>,
    /// `slots = all` adds internal slots to the outcome line; `slots =
    /// public` (the default) leaves them out.
    pub all_slots: bool,
    /// `stored name = Raúl, place = Sevilla, units = metric`, or `stored =
    /// none` (the default): the preferences at the start of each
    /// conversation.
    pub stored: Preferences,
    /// `phrase = tira -> random.dice count=2`, or `phrase = none` (the
    /// default): the user-correction table at the start of each
    /// conversation, one entry per line.
    pub phrases: Vec<Correction>,
    /// `pack = knowledge/seed.toml`, or `pack = none` (the default): a pack
    /// source, relative to the directory that holds the golden directory,
    /// built in memory and given to the engine as its knowledge store. A
    /// build without the `knowledge` feature skips the turns that need one.
    pub pack: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            now: spec_now(),
            seed: 0,
            default_lang: Lang::Es,
            hint: None,
            place: None,
            all_slots: false,
            stored: Preferences::NONE,
            phrases: Vec::new(),
            pack: None,
        }
    }
}

/// 2027-03-14 18:42 Europe/Madrid (UTC+1), a Sunday.
fn spec_now() -> Timestamp {
    Timestamp::from_unix(1_805_046_120).with_utc_offset(3_600)
}

/// One user turn and what is expected of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Case {
    /// 1-based line of the `U:` line.
    pub line: usize,
    /// Whether this turn starts a new conversation.
    pub starts_conversation: bool,
    /// The expected language of the turn (the `[es]`/`[en]` tag).
    pub lang: Lang,
    /// The user's message.
    pub text: String,
    /// The expected reply; continuation lines joined with `\n`.
    pub reply: String,
    /// The expected outcome line, without the arrow.
    pub outcome: String,
    /// The settings in force.
    pub settings: Settings,
    /// Scripted draws for this turn, if a `Given: draw` preceded it.
    pub draws: Option<Vec<Draw>>,
    /// 0-based lines of the `U:` line (first) through the outcome line, for
    /// regeneration.
    lines: Range<usize>,
}

/// A parsed golden file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Golden {
    /// The cases, in order.
    pub cases: Vec<Case>,
    /// The file's lines, for regeneration.
    source: Vec<String>,
}

/// A local wall-clock reading in a named zone or at an offset, as golden
/// files write `now`. The zone database is a Slice C decision; until then
/// a few zones the specification uses are known here with their rules.
fn zone_offset(zone: &str, date: Date, seconds: u32) -> Option<i32> {
    if let Some(offset) = parse_offset(zone) {
        return Some(offset);
    }
    // (standard offset, daylight-saving rule)
    let (standard, rule): (i32, Option<DstRule>) = match zone {
        "Europe/Madrid" | "Europe/Paris" | "Europe/Berlin" | "Europe/Rome" | "CET" => {
            (3_600, Some(eu_rule))
        }
        "Europe/London" | "Europe/Lisbon" | "Atlantic/Canary" => (0, Some(eu_rule)),
        "America/New_York" => (-5 * 3_600, Some(|y| us_rule(y, -5))),
        "America/Chicago" => (-6 * 3_600, Some(|y| us_rule(y, -6))),
        "America/Los_Angeles" => (-8 * 3_600, Some(|y| us_rule(y, -8))),
        "Asia/Tokyo" => (9 * 3_600, None),
        _ => return None,
    };
    let Some(rule) = rule else {
        return Some(standard);
    };
    let (start, end) = rule(date.year());
    let local = date.days_since_epoch() * 86_400 + i64::from(seconds);
    let utc_if_summer = local - i64::from(standard + 3_600);
    Some(if (start..end).contains(&utc_if_summer) {
        standard + 3_600
    } else {
        standard
    })
}

/// A daylight-saving rule: the year's start and end of summer time, as Unix
/// seconds.
type DstRule = fn(i32) -> (i64, i64);

/// `UTC`, `Z`, `GMT`, `+01:00`, `-0400`, `UTC+2`.
fn parse_offset(zone: &str) -> Option<i32> {
    let rest = ["UTC", "GMT", "Z"]
        .iter()
        .find_map(|p| zone.strip_prefix(p))
        .unwrap_or(zone);
    if rest.is_empty() {
        return (zone != rest).then_some(0);
    }
    let sign = match rest.as_bytes()[0] {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let digits = rest[1..].replace(':', "");
    if digits.is_empty() || digits.len() > 4 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let (h, m) = if digits.len() <= 2 {
        (digits.parse::<i32>().ok()?, 0)
    } else {
        let (h, m) = digits.split_at(digits.len() - 2);
        (h.parse::<i32>().ok()?, m.parse::<i32>().ok()?)
    };
    Some(sign * (h * 3_600 + m * 60))
}

/// The `n`th (1-based) or, with `n = 0`, the last Sunday of a month.
fn sunday(year: i32, month: u8, n: u8) -> Date {
    let first = Date::new(year, month, 1).unwrap_or_else(|| Date::from_days_since_epoch(0));
    let to_sunday = 6 - i64::from(first.weekday().days_from_monday());
    let first_sunday = first.add_days(to_sunday);
    if n == 0 {
        let last = first.end_of_month();
        let back = (i64::from(last.weekday().days_from_monday()) + 1) % 7;
        last.add_days(-back)
    } else {
        first_sunday.add_days(7 * i64::from(n - 1))
    }
}

/// EU summer time: last Sunday of March to last Sunday of October, 01:00
/// UTC, as Unix seconds.
fn eu_rule(year: i32) -> (i64, i64) {
    let at = |d: Date| d.days_since_epoch() * 86_400 + 3_600;
    (at(sunday(year, 3, 0)), at(sunday(year, 10, 0)))
}

/// US daylight time: second Sunday of March 02:00 standard to first Sunday
/// of November 02:00 daylight, as Unix seconds.
fn us_rule(year: i32, standard_hours: i64) -> (i64, i64) {
    let start = sunday(year, 3, 2).days_since_epoch() * 86_400 + (2 - standard_hours) * 3_600;
    let end = sunday(year, 11, 1).days_since_epoch() * 86_400 + (1 - standard_hours) * 3_600;
    (start, end)
}

/// `2027-03-14 18:42[:SS] <zone>`.
fn parse_now(value: &str) -> Result<Timestamp, String> {
    let mut parts = value.split_whitespace();
    let (Some(date), Some(time), Some(zone), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(format!(
            "`now = {value}`: expected `YYYY-MM-DD HH:MM[:SS] ZONE`"
        ));
    };
    let numbers =
        |s: &str, sep: char| -> Option<Vec<u32>> { s.split(sep).map(|p| p.parse().ok()).collect() };
    let date = match numbers(date, '-').as_deref() {
        Some(&[y, m, d]) => i32::try_from(y)
            .ok()
            .zip(u8::try_from(m).ok())
            .zip(u8::try_from(d).ok())
            .and_then(|((y, m), d)| Date::new(y, m, d)),
        _ => None,
    }
    .ok_or_else(|| format!("`{date}` is not a date"))?;
    let seconds = match numbers(time, ':').as_deref() {
        Some(&[h, m]) if h < 24 && m < 60 => h * 3_600 + m * 60,
        Some(&[h, m, s]) if h < 24 && m < 60 && s < 60 => h * 3_600 + m * 60 + s,
        _ => return Err(format!("`{time}` is not a time")),
    };
    let offset = zone_offset(zone, date, seconds).ok_or_else(|| {
        format!(
            "unknown zone `{zone}`: write an offset (`+01:00`) or one of the zones in golden.rs"
        )
    })?;
    Ok(Timestamp::from_local(date, seconds, offset))
}

/// Parses `[es] U: text`, returning the tag and the text.
fn parse_user(trimmed: &str) -> Option<(Lang, &str)> {
    let rest = trimmed.strip_prefix('[')?;
    let (tag, rest) = rest.split_once("] U:")?;
    Some((tag.parse().ok()?, rest.trim()))
}

fn is_structural(trimmed: &str) -> bool {
    trimmed.is_empty()
        || trimmed.starts_with('#')
        || trimmed.starts_with("Given:")
        || parse_user(trimmed).is_some()
}

impl Golden {
    /// Parses a golden file.
    ///
    /// # Errors
    ///
    /// A message with the 1-based line of the first thing that is not a
    /// comment, a `Given:` line, a blank line or a well-formed turn.
    pub fn parse(text: &str) -> Result<Golden, String> {
        let source: Vec<String> = text.lines().map(str::to_owned).collect();
        let mut cases = Vec::new();
        let mut settings = Settings::default();
        let mut draws = None;
        let mut starts_conversation = true;
        let mut i = 0;
        while i < source.len() {
            let trimmed = source[i].trim();
            let at = |message: String| format!("line {}: {message}", i + 1);
            if trimmed.is_empty() {
                starts_conversation = true;
                i += 1;
            } else if trimmed.starts_with('#') {
                i += 1;
            } else if let Some(given) = trimmed.strip_prefix("Given:") {
                apply_given(given, &mut settings, &mut draws).map_err(at)?;
                i += 1;
            } else if let Some((lang, text)) = parse_user(trimmed) {
                let first = i;
                i += 1;
                let reply_line = source.get(i).map_or("", |l| l.trim());
                let Some(reply) = reply_line.strip_prefix("T:") else {
                    return Err(format!(
                        "line {}: expected `T:` after the user's turn",
                        i + 1
                    ));
                };
                let mut reply_lines = Vec::new();
                let mut outcome = None;
                let mut pending = Some(reply.trim());
                while let Some(text) = pending.take() {
                    match text.rsplit_once('→') {
                        Some((before, after)) => {
                            if !before.trim().is_empty() {
                                reply_lines.push(before.trim().to_owned());
                            }
                            outcome = Some(after.trim().to_owned());
                        }
                        None => reply_lines.push(text.to_owned()),
                    }
                    i += 1;
                    if outcome.is_none() {
                        let next = source.get(i).map_or("", |l| l.trim());
                        if is_structural(next) {
                            break;
                        }
                        pending = Some(next);
                    }
                }
                let Some(outcome) = outcome else {
                    return Err(format!(
                        "line {}: the turn has no `→` outcome line",
                        first + 1
                    ));
                };
                cases.push(Case {
                    line: first + 1,
                    starts_conversation,
                    lang,
                    text: text.to_owned(),
                    reply: reply_lines.join("\n"),
                    outcome,
                    settings: settings.clone(),
                    draws: draws.take(),
                    lines: first..i,
                });
                starts_conversation = false;
                // Notes after the outcome, up to the next blank line, turn or
                // `Given:`.
                while i < source.len() && !is_structural(source[i].trim()) {
                    i += 1;
                }
            } else {
                return Err(at(format!(
                    "expected `[es] U:`/`[en] U:`, `Given:`, `#` or a blank line, found {trimmed:?}"
                )));
            }
        }
        if draws.is_some() {
            return Err("a `Given: draw` is not followed by a turn".to_owned());
        }
        Ok(Golden { cases, source })
    }
}

fn apply_given(
    given: &str,
    settings: &mut Settings,
    draws: &mut Option<Vec<Draw>>,
) -> Result<(), String> {
    if let Some(list) = given.trim().strip_prefix("stored") {
        settings.stored = parse_stored(list)?;
        return Ok(());
    }
    let Some((key, value)) = given.split_once('=') else {
        return Err(format!("`Given:{given}`: expected `key = value`"));
    };
    let (key, value) = (key.trim(), value.trim());
    let lang = |v: &str| -> Result<Option<Lang>, String> {
        if v == "none" {
            Ok(None)
        } else {
            v.parse()
                .map(Some)
                .map_err(|e: crate::UnknownLang| e.to_string())
        }
    };
    match key {
        "now" => settings.now = parse_now(value)?,
        "seed" => {
            settings.seed = value
                .parse()
                .map_err(|_| format!("`{value}` is not a seed (an unsigned integer)"))?;
        }
        "default language" => {
            settings.default_lang = lang(value)?.ok_or("the default language cannot be none")?;
        }
        "hint" => settings.hint = lang(value)?,
        "place" => settings.place = (value != "none").then(|| Place(value.to_owned())),
        "pack" => settings.pack = (value != "none").then(|| value.to_owned()),
        "phrase" if value == "none" => settings.phrases.clear(),
        "phrase" => settings.phrases.push(parse_phrase(value)?),
        "draw" => *draws = Some(Draw::parse_list(value)?),
        "slots" => {
            settings.all_slots = match value {
                "all" => true,
                "public" => false,
                _ => return Err(format!("`slots = {value}`: expected `all` or `public`")),
            };
        }
        _ => {
            return Err(format!(
                "unknown `Given: {key}` (known: now, seed, default language, hint, place, draw, slots, stored, phrase, pack)"
            ));
        }
    }
    Ok(())
}

/// `name = Raúl, place = Sevilla, units = metric`, or `= none`.
fn parse_stored(list: &str) -> Result<Preferences, String> {
    let list = list.trim();
    if list.trim_start_matches('=').trim() == "none" {
        return Ok(Preferences::NONE);
    }
    let mut stored = Preferences::NONE;
    for item in list.split(',') {
        let Some((key, value)) = item.split_once('=') else {
            return Err(format!("`stored {list}`: expected `key = value, …`"));
        };
        let value = value.trim();
        stored = match key.trim() {
            "name" => stored.with_name(value),
            "place" => stored.with_place(value),
            "units" => stored
                .with_units(Units::parse(value).ok_or_else(|| format!("unknown units `{value}`"))?),
            other => return Err(format!("`stored {other}`: expected name, place or units")),
        };
    }
    Ok(stored)
}

/// `tira -> random.dice count=2`: a taught phrase, its intent and slots.
fn parse_phrase(value: &str) -> Result<Correction, String> {
    let Some((phrase, meaning)) = value.split_once("->") else {
        return Err(format!(
            "`phrase = {value}`: expected `phrase -> intent slot=value …`"
        ));
    };
    let mut words = meaning.split_whitespace();
    let intent = words.next().ok_or("a phrase needs an intent")?;
    let mut slots = std::collections::BTreeMap::new();
    for word in words {
        let (k, v) = word
            .split_once('=')
            .ok_or_else(|| format!("`{word}`: expected `slot=value`"))?;
        slots.insert(k.to_owned(), v.to_owned());
    }
    Ok(Correction::new(phrase, intent, slots))
}

/// A case that did not come out as expected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    /// The file.
    pub file: PathBuf,
    /// 1-based line of the turn.
    pub line: usize,
    /// What differed, readably.
    pub message: String,
}

/// What a run of golden files found.
#[derive(Debug, Default)]
pub struct Summary {
    /// Files read.
    pub files: usize,
    /// Turns run.
    pub turns: usize,
    /// Cases that differed (none when regenerating).
    pub failures: Vec<Failure>,
    /// Files rewritten by a regeneration.
    pub rewritten: Vec<PathBuf>,
    /// Turns not run: they need a knowledge pack and this build has no
    /// knowledge store.
    pub skipped: usize,
}

impl Summary {
    /// A readable report of the failures.
    #[must_use]
    pub fn report(&self) -> String {
        let mut out = String::new();
        for f in &self.failures {
            let _ = writeln!(out, "{}:{}:\n{}\n", f.file.display(), f.line, f.message);
        }
        let _ = write!(
            out,
            "{} of {} golden turns in {} files differ; to accept the new output, \
             run with TALOS_BLESS=1 and review the diff",
            self.failures.len(),
            self.turns,
            self.files
        );
        out
    }
}

/// Builds engines for golden runs, one per configuration.
pub type Build<'a> = &'a dyn Fn(Config) -> Result<Engine, Error>;

/// The T block of a case: the reply, then the outcome at [`ARROW_COLUMN`].
fn format_reply(reply: &str, outcome: &str) -> Vec<String> {
    let mut lines: Vec<String> = reply
        .split('\n')
        .enumerate()
        .map(|(k, l)| {
            if k == 0 {
                format!("     T: {l}")
            } else {
                format!("        {l}")
            }
        })
        .collect();
    let width = lines.last().map_or(0, |l| l.chars().count());
    if lines.len() == 1 && width < ARROW_COLUMN - 1 {
        let pad = ARROW_COLUMN - width;
        lines[0].push_str(&" ".repeat(pad));
        lines[0].push_str("→ ");
        lines[0].push_str(outcome);
    } else {
        lines.push(format!("{}→ {outcome}", " ".repeat(ARROW_COLUMN)));
    }
    lines
}

impl Settings {
    /// What is stored about the user at the start of a conversation: set
    /// it with [`Engine::set_user_data`] whenever a case starts one.
    #[must_use]
    pub fn user_data(&self) -> UserData {
        let mut stored = UserData {
            preferences: self.stored.clone(),
            corrections: Vec::new(),
        };
        for phrase in &self.phrases {
            stored.correct(phrase.clone());
        }
        stored
    }
}

/// Runs one parsed golden file. With `bless`, returns the file's text with
/// every T block regenerated instead of failures.
/// The engines a run has built, by configuration and pack.
type Engines = Vec<((Config, Option<String>), Engine)>;

/// An engine for a configuration and a pack source (relative to `root`).
fn engine_for(
    build: Build<'_>,
    config: Config,
    pack: Option<&str>,
    root: &Path,
) -> Result<Engine, String> {
    let engine = build(config).map_err(|e| e.to_string())?;
    let Some(pack) = pack else {
        return Ok(engine);
    };
    #[cfg(feature = "knowledge")]
    {
        let path = root.join(pack);
        let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let knowledge = crate::knowledge::Knowledge::from_source(&text)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let mut engine = engine;
        engine.use_knowledge(knowledge);
        Ok(engine)
    }
    #[cfg(not(feature = "knowledge"))]
    {
        let _ = (engine, root);
        Err(format!("`pack = {pack}` needs the `knowledge` feature"))
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "one case run and compared in the order it is read"
)]
fn run_golden(
    file: &Path,
    golden: &Golden,
    (build, root): (Build<'_>, &Path),
    engines: &mut Engines,
    summary: &mut Summary,
    bless: bool,
) -> Result<Option<String>, String> {
    let conversation = ConvId::new("golden");
    let mut replacements: Vec<(Range<usize>, Vec<String>)> = Vec::new();
    for case in &golden.cases {
        let s = &case.settings;
        if s.pack.is_some() && !cfg!(feature = "knowledge") {
            summary.skipped += 1;
            continue;
        }
        let config = Config {
            default_lang: s.default_lang,
            seed: s.seed,
            ..Config::default()
        };
        let key = (config, s.pack.clone());
        let index = if let Some(index) = engines.iter().position(|(k, _)| *k == key) {
            index
        } else {
            let engine = engine_for(build, config, s.pack.as_deref(), root)?;
            engines.push((key, engine));
            engines.len() - 1
        };
        let engine = &engines[index].1;
        if case.starts_conversation {
            engine.end(&conversation);
            engine
                .set_user_data(s.user_data())
                .map_err(|e| e.to_string())?;
        }
        let mut turn = Turn::new(case.text.clone(), conversation.clone(), s.now)
            .with_lang(s.hint)
            .with_place(s.place.clone());
        if s.all_slots {
            turn = turn.with_trace();
        }
        let (outcome, report) = match &case.draws {
            Some(draws) => {
                let (outcome, report) = engine.respond_scripted(turn, draws.clone());
                (outcome, Some(report))
            }
            None => (engine.respond(turn), None),
        };
        let internal = outcome.trace.as_ref().map(|t| &t.internal_slots);
        let line = outcome.line(if s.all_slots { internal } else { None });
        summary.turns += 1;
        if bless {
            let mut block = vec![
                format!("[{}] U: {}", outcome.lang, case.text)
                    .trim_end()
                    .to_owned(),
            ];
            block.extend(format_reply(&outcome.reply, &line));
            replacements.push((case.lines.clone(), block));
            continue;
        }
        let mut problems = Vec::new();
        if outcome.lang != case.lang {
            problems.push(format!(
                "  language: expected [{}], got [{}]",
                case.lang, outcome.lang
            ));
        }
        if outcome.reply != case.reply || line != case.outcome {
            problems.push(format!(
                "  expected: {}  → {}\n  actual:   {}  → {}",
                case.reply.replace('\n', "\\n"),
                case.outcome,
                outcome.reply.replace('\n', "\\n"),
                line
            ));
        }
        if let Some(report) = report {
            for error in &report.errors {
                problems.push(format!("  draw: {error}"));
            }
            if !report.unused.is_empty() {
                let unused: Vec<String> = report.unused.iter().map(ToString::to_string).collect();
                problems.push(format!("  draw: not drawn: {}", unused.join(", ")));
            }
        }
        if !problems.is_empty() {
            summary.failures.push(Failure {
                file: file.to_owned(),
                line: case.line,
                message: format!("[{}] U: {}\n{}", case.lang, case.text, problems.join("\n")),
            });
        }
    }
    if !bless {
        return Ok(None);
    }
    let mut out: Vec<String> = Vec::with_capacity(golden.source.len());
    let mut next = 0;
    for (range, block) in replacements {
        out.extend(golden.source[next..range.start].iter().cloned());
        out.extend(block);
        next = range.end;
    }
    out.extend(golden.source[next..].iter().cloned());
    let mut text = out.join("\n");
    text.push('\n');
    Ok(Some(text))
}

/// Every `.txt` file under `dir`, recursively, sorted.
fn files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut found = Vec::new();
    let entries = fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_dir() {
            found.extend(files(&path)?);
        } else if path.extension().is_some_and(|e| e == "txt") {
            found.push(path);
        }
    }
    found.sort();
    Ok(found)
}

/// Runs every golden file under `dir` against engines made by `build`. With
/// `bless`, rewrites each file whose output changed instead of failing.
///
/// # Errors
///
/// A file that cannot be read, parsed or written, or an engine that cannot
/// be built.
pub fn run_dir(dir: &Path, build: Build<'_>, bless: bool) -> Result<Summary, String> {
    let mut summary = Summary::default();
    let mut engines = Vec::new();
    let root = dir.parent().unwrap_or(dir);
    for file in files(dir)? {
        let text = fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        let golden = Golden::parse(&text).map_err(|e| format!("{}: {e}", file.display()))?;
        summary.files += 1;
        let run = (build, root);
        if let Some(new) = run_golden(&file, &golden, run, &mut engines, &mut summary, bless)?
            && new != text
        {
            fs::write(&file, new).map_err(|e| format!("{}: {e}", file.display()))?;
            summary.rewritten.push(file);
        }
    }
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::civil::Weekday;

    const SAMPLE: &str = "\
# Greetings.
Given: now = 2027-03-14 09:42 Europe/Madrid

[es] U: hola
     T: ¡Hola!                                   → answered · smalltalk.greet
     (a note
      over two lines)
[es] U: buenas
     T: ¡Buenas!
                                                  → answered · smalltalk.greet

Given: hint = en
Given: draw = 3, option 2
[en] U: list
     T: Here:
        · one
        · two
        → answered · help.capabilities
";

    #[test]
    fn parses_conversations_settings_and_notes() {
        let g = Golden::parse(SAMPLE).unwrap();
        assert_eq!(g.cases.len(), 3);
        let [a, b, c] = &g.cases[..] else { panic!() };
        assert!(a.starts_conversation && !b.starts_conversation && c.starts_conversation);
        assert_eq!((a.line, a.lang, a.text.as_str()), (4, Lang::Es, "hola"));
        assert_eq!(a.reply, "¡Hola!");
        assert_eq!(a.outcome, "answered · smalltalk.greet");
        assert_eq!(a.settings.now.local_hour(), Some(9));
        assert_eq!(b.reply, "¡Buenas!");
        assert_eq!(b.lines, 7..10);
        assert_eq!(c.reply, "Here:\n· one\n· two");
        assert_eq!(c.settings.hint, Some(Lang::En));
        assert_eq!(c.draws.as_ref().map(Vec::len), Some(2));
        assert_eq!(a.draws, None);
    }

    #[test]
    fn reports_malformed_files_by_line() {
        let err = |text: &str| Golden::parse(text).unwrap_err();
        assert!(err("[es] U: hola\n").starts_with("line 2: expected `T:`"));
        assert!(err("[es] U: hola\n     T: ¡Hola!\n").starts_with("line 1: the turn has no"));
        assert!(err("Given: mood = happy\n").contains("unknown `Given: mood`"));
        assert!(err("Given: now = 2027-03-14 18:42 Mars/Olympus\n").contains("unknown zone"));
        assert!(err("hola\n").starts_with("line 1: expected"));
        assert!(err("Given: draw = 4\n").contains("not followed by a turn"));
    }

    #[test]
    fn zones_follow_their_rules() {
        let offset = |text: &str| parse_now(text).unwrap().utc_offset_seconds;
        assert_eq!(offset("2027-03-14 18:42 Europe/Madrid"), Some(3_600));
        assert_eq!(offset("2027-07-01 12:00 Europe/Madrid"), Some(7_200));
        assert_eq!(offset("2027-03-28 03:30 Europe/Madrid"), Some(7_200));
        assert_eq!(offset("2027-03-28 01:30 Europe/Madrid"), Some(3_600));
        assert_eq!(
            offset("2027-03-14 18:42 America/New_York"),
            Some(-4 * 3_600)
        );
        assert_eq!(
            offset("2027-03-13 18:42 America/New_York"),
            Some(-5 * 3_600)
        );
        assert_eq!(
            offset("2027-11-07 12:00 America/New_York"),
            Some(-5 * 3_600)
        );
        assert_eq!(offset("2027-03-14 18:42 UTC"), Some(0));
        assert_eq!(offset("2027-03-14 18:42 +05:30"), Some(19_800));
        assert_eq!(offset("2027-03-14 18:42 UTC-3"), Some(-10_800));
        assert_eq!(offset("2027-03-14 18:42:59 Asia/Tokyo"), Some(32_400));
        assert_eq!(sunday(2027, 3, 0).weekday(), Weekday::Sunday);
        assert_eq!(sunday(2027, 3, 0).day(), 28);
        assert_eq!(sunday(2027, 3, 2).day(), 14);
        assert_eq!(sunday(2027, 11, 1).day(), 7);
        let now = parse_now("2027-03-14 18:42 Europe/Madrid").unwrap();
        assert_eq!(now, spec_now());
    }

    #[test]
    fn replies_are_laid_out_like_the_spec() {
        assert_eq!(
            format_reply("¡Hola!", "answered · smalltalk.greet"),
            [format!(
                "     T: ¡Hola!{}→ answered · smalltalk.greet",
                " ".repeat(36)
            )]
        );
        let long = format_reply("Here:\n· one", "answered");
        assert_eq!(long.len(), 3);
        assert_eq!(long[1], "        · one");
        assert!(long[2].ends_with("→ answered"));
    }
}
