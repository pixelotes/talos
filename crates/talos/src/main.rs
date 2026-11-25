//! The `talos` binary: the single entry point for the chat, ask and serve
//! front ends (DESIGN.md section 2, docs/API.md section 1). Each front end is
//! a thin shell around `Engine::respond`.

use std::fmt::Write as _;
use std::io::{self, BufRead, IsTerminal, Write};
use std::process::ExitCode;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value as Json, json};
use talos_core::check::Severity;
use talos_core::{Config, ConvId, Engine, Lang, Outcome, Status, Timestamp, Turn, Value};

const VERSION: &str = env!("CARGO_PKG_VERSION");

const USAGE: &str = "\
usage: talos [options] ask <text>        answer one message, then exit
       talos [options] chat              a conversation on stdin/stdout
       talos [options] explain <text>    show how a message is answered, stage by stage
       talos skills list | check         inspect and validate the skill manifests
       talos --version

options: --lang es|en   the caller's language hint
         --seed N       the random source's seed (default 0)
         --json         print the full outcome object (ask, explain)

exit status of `ask`: 0 answered, 2 deflected, 3 unhandled;
of `skills check`: 1 when a manifest has errors";

/// Wrong arguments (BSD `EX_USAGE`).
const EXIT_USAGE: u8 = 64;
/// The engine could not be built (BSD `EX_SOFTWARE`).
const EXIT_SOFTWARE: u8 = 70;
/// Reading or writing the terminal failed (BSD `EX_IOERR`).
const EXIT_IO: u8 = 74;

/// Flags shared by the turn commands.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Options {
    lang: Option<Lang>,
    seed: u64,
    json: bool,
}

/// What the command line asked for.
#[derive(Debug, PartialEq, Eq)]
enum Command {
    Version,
    Help,
    Ask(Options, String),
    Explain(Options, String),
    Chat(Options),
    SkillsList,
    SkillsCheck,
}

/// Parses the arguments after the program name. Flags may come before or
/// after the subcommand; `--` ends flags.
fn parse_args(args: &[String]) -> Result<Command, String> {
    let mut options = Options::default();
    let mut positional: Vec<&str> = Vec::new();
    let mut iter = args.iter();
    let mut flags_done = false;
    while let Some(arg) = iter.next() {
        let arg = arg.as_str();
        if flags_done || !arg.starts_with("--") && !matches!(arg, "-V" | "-h") {
            positional.push(arg);
            continue;
        }
        let (name, inline) = match arg.split_once('=') {
            Some((name, value)) => (name, Some(value)),
            None => (arg, None),
        };
        let mut value = |what: &str| -> Result<&str, String> {
            inline
                .or_else(|| iter.next().map(String::as_str))
                .ok_or_else(|| format!("`{name}` needs a value: {what}"))
        };
        match name {
            "--" => flags_done = true,
            "--version" | "-V" => return Ok(Command::Version),
            "--help" | "-h" => return Ok(Command::Help),
            "--json" => options.json = true,
            "--lang" => {
                options.lang = Some(
                    value("es or en")?
                        .parse::<Lang>()
                        .map_err(|e| e.to_string())?,
                );
            }
            "--seed" => {
                let seed = value("an unsigned integer")?;
                options.seed = seed
                    .parse()
                    .map_err(|_| format!("`--seed {seed}`: expected an unsigned integer"))?;
            }
            _ => return Err(format!("unknown option `{arg}`")),
        }
    }
    match positional.split_first() {
        Some((&"ask", words)) if !words.is_empty() => Ok(Command::Ask(options, words.join(" "))),
        Some((&"explain", words)) if !words.is_empty() => {
            Ok(Command::Explain(options, words.join(" ")))
        }
        Some((&("ask" | "explain"), _)) => Err(format!("`{}` needs the text", positional[0])),
        Some((&"chat", [])) => Ok(Command::Chat(options)),
        Some((&"chat", _)) => Err("`chat` takes no text; type it after it starts".to_owned()),
        Some((&"skills", ["list"])) => Ok(Command::SkillsList),
        Some((&"skills", ["check"])) => Ok(Command::SkillsCheck),
        Some((&"skills", _)) => Err("`skills` needs `list` or `check`".to_owned()),
        Some((other, _)) => Err(format!("unknown command `{other}`")),
        None => Ok(Command::Help),
    }
}

/// The machine's current offset from UTC, by the smallest means that needs
/// neither `unsafe` nor a date/time crate: the `date` utility, which every
/// Unix has and which honours `TZ`. `None` where it is missing (Windows,
/// minimal containers); time-of-day answers then stay time-neutral.
fn local_offset() -> Option<i32> {
    let output = std::process::Command::new("date")
        .arg("+%z")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_offset(std::str::from_utf8(&output.stdout).ok()?.trim())
}

/// `+0100`, `-0430`.
fn parse_offset(text: &str) -> Option<i32> {
    let sign = match text.as_bytes().first()? {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let digits = &text[1..];
    if digits.len() != 4 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let hours: i32 = digits[..2].parse().ok()?;
    let minutes: i32 = digits[2..].parse().ok()?;
    Some(sign * (hours * 3_600 + minutes * 60))
}

/// The current instant, read here in the front end before each turn; the
/// engine never reads the clock.
fn now() -> Timestamp {
    let seconds = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(after) => i64::try_from(after.as_secs()).unwrap_or(i64::MAX),
        Err(before) => i64::try_from(before.duration().as_secs()).map_or(i64::MIN, |s| -s),
    };
    let now = Timestamp::from_unix(seconds);
    match local_offset() {
        Some(offset) => now.with_utc_offset(offset),
        None => now,
    }
}

/// The exit status of `talos ask` for an outcome.
fn exit_status(status: Status) -> u8 {
    match status {
        Status::Answered => 0,
        Status::Deflected => 2,
        Status::Unhandled => 3,
    }
}

/// A slot value on the wire: a list as an array of strings, anything else
/// as its canonical text (numbers with a decimal point, dates in ISO 8601),
/// so that no number loses exactness.
fn slot_json(value: &Value) -> Json {
    match value {
        Value::List { items, .. } => json!(items),
        other => json!(other.to_string()),
    }
}

/// The full outcome with the field names of the `talos` object of
/// docs/API.md section 2.
fn outcome_json(outcome: &Outcome, elapsed_us: u128) -> Json {
    let slots: Map<String, Json> = outcome
        .slots
        .iter()
        .map(|(name, value)| (name.clone(), slot_json(value)))
        .collect();
    let mut object = json!({
        "reply": outcome.reply,
        "status": outcome.status.as_str(),
        "intent": outcome.intent.as_ref().map(talos_core::IntentId::as_str),
        "slots": slots,
        "confidence": outcome.confidence,
        "language": outcome.lang.code(),
        "reason": outcome.reason.as_ref().map(talos_core::Reason::as_str),
        "pending": outcome.pending,
        "generated": outcome.generated,
        "elapsed_us": u64::try_from(elapsed_us).unwrap_or(u64::MAX),
    });
    if let (Some(trace), Some(map)) = (&outcome.trace, object.as_object_mut()) {
        let steps: Vec<Json> = trace
            .steps
            .iter()
            .map(|s| json!({ "stage": s.stage, "note": s.note }))
            .collect();
        let internal: Map<String, Json> = trace
            .internal_slots
            .iter()
            .map(|(name, value)| (name.clone(), slot_json(value)))
            .collect();
        map.insert("trace".to_owned(), json!(steps));
        map.insert("internal_slots".to_owned(), json!(internal));
    }
    object
}

/// Runs one turn of a one-shot command and prints it.
fn one_shot(engine: &Engine, options: Options, text: String, explain: bool) -> ExitCode {
    let conversation = ConvId::new(if explain { "explain" } else { "ask" });
    let mut turn = Turn::new(text, conversation, now()).with_lang(options.lang);
    if explain {
        turn = turn.with_trace();
    }
    let started = Instant::now();
    let outcome = engine.respond(turn);
    let elapsed = started.elapsed().as_micros();
    if options.json {
        println!("{:#}", outcome_json(&outcome, elapsed));
    } else if explain {
        print!("{}", explanation(&outcome));
    } else {
        println!("{}", outcome.reply);
    }
    if explain {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(exit_status(outcome.status))
    }
}

/// `talos explain`: the trace, one stage per line, then the result.
fn explanation(outcome: &Outcome) -> String {
    let mut lines: Vec<(&str, String)> = Vec::new();
    if let Some(trace) = &outcome.trace {
        lines.extend(trace.steps.iter().map(|s| (s.stage, s.note.clone())));
        if !trace.internal_slots.is_empty() {
            let internal: Vec<String> = trace
                .internal_slots
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect();
            lines.push(("internal", internal.join(" ")));
        }
    }
    lines.push((
        "outcome",
        format!("[{}] {}", outcome.lang, outcome.line(None)),
    ));
    lines.push(("reply", outcome.reply.replace('\n', "\n           ")));
    let mut out = String::new();
    for (stage, note) in &lines {
        let _ = writeln!(out, "{stage:<10} {note}");
    }
    out
}

/// "1 skill", "2 skills".
fn plural(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("1 {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

/// `talos skills list`.
fn skills_list(engine: &Engine) {
    for m in engine.skills() {
        let patterns = |lang: Lang| {
            m.intents
                .iter()
                .flat_map(|i| &i.patterns)
                .filter(|p| p.lang == lang)
                .count()
        };
        println!(
            "{:<12} {} / {}\n{:<12} {}, {} es + {} en patterns, {}",
            m.id,
            m.summary.en,
            m.summary.es,
            "",
            plural(m.intents.len(), "intent"),
            patterns(Lang::Es),
            patterns(Lang::En),
            plural(m.replies.len(), "reply id")
        );
    }
}

/// `talos skills check`.
fn skills_check() -> ExitCode {
    let report = talos_skills::check();
    for finding in &report.findings {
        println!("{finding}");
    }
    let errors = report.count(Severity::Error);
    let warnings = report.count(Severity::Warning);
    println!(
        "{} checked: {}, {}",
        plural(report.skills, "skill"),
        plural(errors, "error"),
        plural(warnings, "warning")
    );
    if errors > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// One conversation over line-based input: one turn per non-blank line, one
/// reply line each, until end of input.
fn chat(
    engine: &Engine,
    lang: Option<Lang>,
    clock: impl Fn() -> Timestamp,
    input: impl BufRead,
    mut output: impl Write,
    prompt: bool,
) -> io::Result<()> {
    let conversation = ConvId::new("chat");
    if prompt {
        write!(output, "> ")?;
        output.flush()?;
    }
    for line in input.lines() {
        let line = line?;
        if !line.trim().is_empty() {
            let turn = Turn::new(line, conversation.clone(), clock()).with_lang(lang);
            writeln!(output, "{}", engine.respond(turn).reply)?;
        }
        if prompt {
            write!(output, "> ")?;
            output.flush()?;
        }
    }
    if prompt {
        writeln!(output)?;
    }
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = match parse_args(&args) {
        Ok(command) => command,
        Err(message) => {
            eprintln!("talos: {message}\n\n{USAGE}");
            return ExitCode::from(EXIT_USAGE);
        }
    };
    let options = match &command {
        Command::Version => {
            println!("talos {VERSION}");
            return ExitCode::SUCCESS;
        }
        Command::Help => {
            println!("talos {VERSION}\n\n{USAGE}");
            return ExitCode::SUCCESS;
        }
        Command::SkillsCheck => return skills_check(),
        Command::Ask(options, _) | Command::Explain(options, _) | Command::Chat(options) => {
            *options
        }
        Command::SkillsList => Options::default(),
    };
    let config = Config {
        seed: options.seed,
        ..Config::default()
    };
    let engine = match talos_skills::engine(config) {
        Ok(engine) => engine,
        Err(error) => {
            eprintln!("talos: {error}");
            return ExitCode::from(EXIT_SOFTWARE);
        }
    };
    match command {
        Command::Ask(options, text) => one_shot(&engine, options, text, false),
        Command::Explain(options, text) => one_shot(&engine, options, text, true),
        Command::SkillsList => {
            skills_list(&engine);
            ExitCode::SUCCESS
        }
        _ => {
            let stdin = io::stdin();
            let prompt = stdin.is_terminal();
            match chat(
                &engine,
                options.lang,
                now,
                stdin.lock(),
                io::stdout().lock(),
                prompt,
            ) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("talos: {error}");
                    ExitCode::from(EXIT_IO)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    fn parse(list: &[&str]) -> Result<Command, String> {
        parse_args(&args(list))
    }

    fn lang(lang: Lang) -> Options {
        Options {
            lang: Some(lang),
            ..Options::default()
        }
    }

    #[test]
    fn version_and_help() {
        assert_eq!(parse(&["--version"]), Ok(Command::Version));
        assert_eq!(parse(&["-V"]), Ok(Command::Version));
        assert_eq!(parse(&["--help"]), Ok(Command::Help));
        assert_eq!(parse(&[]), Ok(Command::Help));
    }

    #[test]
    fn ask_with_flags_before_or_after_the_command() {
        let expected = Ok(Command::Ask(lang(Lang::En), "hola".to_owned()));
        assert_eq!(parse(&["--lang", "en", "ask", "hola"]), expected);
        assert_eq!(parse(&["ask", "--lang", "en", "hola"]), expected);
        assert_eq!(parse(&["ask", "--lang=en", "hola"]), expected);
        assert_eq!(
            parse(&["ask", "qué", "hora", "es"]),
            Ok(Command::Ask(Options::default(), "qué hora es".to_owned()))
        );
        assert_eq!(
            parse(&["ask", "--", "--lang"]),
            Ok(Command::Ask(Options::default(), "--lang".to_owned()))
        );
        assert_eq!(
            parse(&["--json", "--seed", "7", "ask", "hola"]),
            Ok(Command::Ask(
                Options {
                    lang: None,
                    seed: 7,
                    json: true
                },
                "hola".to_owned()
            ))
        );
    }

    #[test]
    fn other_commands_and_errors() {
        assert_eq!(
            parse(&["chat", "--lang", "es"]),
            Ok(Command::Chat(lang(Lang::Es)))
        );
        assert_eq!(
            parse(&["explain", "buenos", "días"]),
            Ok(Command::Explain(
                Options::default(),
                "buenos días".to_owned()
            ))
        );
        assert_eq!(parse(&["skills", "list"]), Ok(Command::SkillsList));
        assert_eq!(parse(&["skills", "check"]), Ok(Command::SkillsCheck));
        assert!(parse(&["skills"]).is_err());
        assert!(parse(&["ask"]).is_err());
        assert!(parse(&["explain"]).is_err());
        assert!(parse(&["chat", "hola"]).is_err());
        assert!(parse(&["ask", "--lang", "fr", "x"]).is_err());
        assert!(parse(&["ask", "--lang"]).is_err());
        assert!(parse(&["ask", "--seed", "-1", "x"]).is_err());
        assert!(parse(&["--frobnicate"]).is_err());
        assert!(parse(&["serve"]).is_err());
    }

    #[test]
    fn exit_statuses_follow_the_api() {
        assert_eq!(exit_status(Status::Answered), 0);
        assert_eq!(exit_status(Status::Deflected), 2);
        assert_eq!(exit_status(Status::Unhandled), 3);
    }

    #[test]
    fn utc_offsets_from_date() {
        assert_eq!(parse_offset("+0100"), Some(3_600));
        assert_eq!(parse_offset("-0430"), Some(-16_200));
        assert_eq!(parse_offset("+0000"), Some(0));
        assert_eq!(parse_offset("CET"), None);
        assert_eq!(parse_offset("+01"), None);
    }

    #[test]
    fn the_json_outcome_uses_the_api_field_names() {
        let engine = talos_skills::engine(Config::default()).unwrap();
        let turn = Turn::new("hello", ConvId::new("t"), Timestamp::from_unix(0));
        let json = outcome_json(&engine.respond(turn), 12);
        assert_eq!(json["status"], "answered");
        assert_eq!(json["intent"], "smalltalk.greet");
        assert_eq!(json["language"], "en");
        assert_eq!(json["confidence"], 1.0);
        assert_eq!(json["reason"], Json::Null);
        assert_eq!(json["pending"], false);
        assert_eq!(json["generated"], false);
        assert_eq!(json["elapsed_us"], 12);
        assert_eq!(json["reply"], "Hello!");
        assert!(json["slots"].as_object().unwrap().is_empty());
        assert!(json.get("trace").is_none());
    }

    #[test]
    fn chat_keeps_one_conversation() {
        let engine = talos_skills::engine(Config::default()).unwrap();
        // Spanish, then one English word (stays Spanish), then a clear English
        // phrase (switches), then one Spanish word (stays English), then a
        // clear Spanish phrase (switches back).
        let input = "hola\n\n  \nhello\ngood morning talos\nhola\nescríbeme un poema\n";
        let mut output = Vec::new();
        let clock = || Timestamp::from_unix(1_805_046_120);
        chat(&engine, None, clock, input.as_bytes(), &mut output, false).unwrap();
        let output = String::from_utf8(output).unwrap();
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 5);
        assert_eq!(lines[0], "¡Hola!");
        assert!(lines[1].starts_with("¡Hola"), "{output}");
        assert!(["Hello!", "Hi!"].contains(&lines[2]), "{output}");
        assert!(["Hello!", "Hi!"].contains(&lines[3]), "{output}");
        assert_eq!(lines[4], "No sé escribir textos nuevos.");
    }
}
