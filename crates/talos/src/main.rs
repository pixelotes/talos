//! The `talos` binary: the single entry point for the chat, ask and serve
//! front ends (DESIGN.md section 2, docs/API.md section 1). Each front end is
//! a thin shell around `Engine::respond`.

use std::fmt::Write as _;
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use talos::VERSION;
use talos::clock::now;
use talos::wire::outcome_json;
use talos_core::check::Severity;
use talos_core::{Config, ConvId, Engine, Lang, Outcome, Status, Timestamp, Turn};

mod eval;

const USAGE: &str = "\
usage: talos [options] ask <text>        answer one message, then exit
       talos [options] chat              a conversation on stdin/stdout
       talos [options] explain <text>    show how a message is answered, stage by stage
       talos skills list | check         inspect and validate the skill manifests
       talos packs list                  the knowledge packs found, and any left out
       talos packs info | check <file>   what a pack holds; verify it whole
       talos eval                        score the batteries, rules only and with the classifier
       talos [options] serve             the OpenAI-compatible HTTP server
       talos --version

options: --lang es|en   the caller's language hint
         --seed N       the random source's seed (default 0)
         --json         print the full outcome object (ask, explain) or the report (eval)
         --dir PATH     the batteries' directory (eval; default `evals`)
         --bind ADDR    the address to listen on (serve; default 127.0.0.1)
         --port N       the port (serve; default 8099, 0 for any free one)
         --token TOKEN  the bearer token clients must send (serve; default
                        $TALOS_TOKEN); required to bind a non-loopback address

exit status of `ask`: 0 answered, 2 deflected, 3 unhandled;
of `skills check`: 1 when a manifest has errors;
of `packs`: 1 when a pack is refused or has errors";

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
    Packs(Packs),
    Eval(Options, PathBuf),
    Serve(Options, Serve),
}

/// `talos packs …`.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Packs {
    List,
    Info(PathBuf),
    Check(PathBuf),
}

/// The flags of `talos serve`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Serve {
    bind: Option<std::net::IpAddr>,
    port: Option<u16>,
    token: Option<String>,
}

/// Parses the arguments after the program name. Flags may come before or
/// after the subcommand; `--` ends flags.
fn parse_args(args: &[String]) -> Result<Command, String> {
    let mut options = Options::default();
    let mut positional: Vec<&str> = Vec::new();
    let mut iter = args.iter();
    let mut flags_done = false;
    let mut dir: Option<&str> = None;
    let mut serve = Serve::default();
    let mut serve_flags = false;
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
            "--dir" => dir = Some(value("a directory")?),
            "--bind" => {
                let addr = value("an IP address")?;
                serve.bind = Some(if addr == "localhost" {
                    std::net::IpAddr::from([127, 0, 0, 1])
                } else {
                    addr.parse()
                        .map_err(|_| format!("`--bind {addr}`: expected an IP address"))?
                });
                serve_flags = true;
            }
            "--port" => {
                let port = value("a port number")?;
                serve.port = Some(
                    port.parse()
                        .map_err(|_| format!("`--port {port}`: expected a port number"))?,
                );
                serve_flags = true;
            }
            "--token" => {
                serve.token = Some(value("a token")?.to_owned());
                serve_flags = true;
            }
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
    if dir.is_some() && positional.first() != Some(&"eval") {
        return Err("`--dir` is only for `eval`".to_owned());
    }
    if serve_flags && positional.first() != Some(&"serve") {
        return Err("`--bind`, `--port` and `--token` are only for `serve`".to_owned());
    }
    match positional.split_first() {
        Some((&"eval", [])) => Ok(Command::Eval(
            options,
            PathBuf::from(dir.unwrap_or("evals")),
        )),
        Some((&"eval", _)) => Err("`eval` takes no text; use `--dir PATH`".to_owned()),
        Some((&"ask", words)) if !words.is_empty() => Ok(Command::Ask(options, words.join(" "))),
        Some((&"explain", words)) if !words.is_empty() => {
            Ok(Command::Explain(options, words.join(" ")))
        }
        Some((&("ask" | "explain"), _)) => Err(format!("`{}` needs the text", positional[0])),
        Some((&"chat", [])) => Ok(Command::Chat(options)),
        Some((&"chat", _)) => Err("`chat` takes no text; type it after it starts".to_owned()),
        Some((&"serve", [])) => Ok(Command::Serve(options, serve)),
        Some((&"serve", _)) => Err("`serve` takes no text".to_owned()),
        Some((&"skills", ["list"])) => Ok(Command::SkillsList),
        Some((&"skills", ["check"])) => Ok(Command::SkillsCheck),
        Some((&"skills", _)) => Err("`skills` needs `list` or `check`".to_owned()),
        Some((&"packs", ["list"])) => Ok(Command::Packs(Packs::List)),
        Some((&"packs", ["info", file])) => Ok(Command::Packs(Packs::Info(PathBuf::from(file)))),
        Some((&"packs", ["check", file])) => Ok(Command::Packs(Packs::Check(PathBuf::from(file)))),
        Some((&"packs", _)) => Err("`packs` needs `list`, `info FILE` or `check FILE`".to_owned()),
        Some((other, _)) => Err(format!("unknown command `{other}`")),
        None => Ok(Command::Help),
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

/// `talos skills check`. A classifier model that is missing, damaged or
/// trained on other manifests is a warning: the engine runs on rules alone.
fn skills_check() -> ExitCode {
    let report = talos_skills::check();
    for finding in &report.findings {
        println!("{finding}");
    }
    let errors = report.count(Severity::Error);
    let mut warnings = report.count(Severity::Warning);
    match talos_skills::engine(Config::default()) {
        Ok(engine) => {
            let c = engine.classifiers();
            for (name, model) in [
                ("intent", c.intent_model()),
                ("language", c.language_model()),
            ] {
                match model.map(|m| (m, m.verify())) {
                    Ok((m, Ok(()))) => println!(
                        "{name} model: {} classes, {} features, {} bytes",
                        m.classes().len(),
                        m.vocabulary_len(),
                        m.file_len()
                    ),
                    Ok((_, Err(error))) => {
                        warnings += 1;
                        println!("warning: {name} model damaged: {error}");
                    }
                    Err(why) => {
                        warnings += 1;
                        println!("warning: {name} model off: {why}");
                    }
                }
            }
        }
        Err(error) => println!("warning: engine not built: {error}"),
    }
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

/// `talos eval`: exits 0 whatever the scores, as long as the batteries ran.
/// Compares the engine with a rules-only one built with the same settings.
fn run_eval(engine: &Engine, options: Options, dir: &std::path::Path) -> ExitCode {
    let rules = match talos_skills::rules_only_engine(engine.config()) {
        Ok(rules) => rules,
        Err(error) => {
            eprintln!("talos: {error}");
            return ExitCode::from(EXIT_SOFTWARE);
        }
    };
    match eval::run(engine, &rules, dir) {
        Ok(report) => {
            if options.json {
                println!("{:#}", eval::render_json(&report));
            } else {
                print!("{}", eval::render_text(&report));
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("talos: {error}");
            ExitCode::from(EXIT_IO)
        }
    }
}

/// `talos serve`: runs until the process is stopped.
#[cfg(feature = "server")]
fn serve(engine: Engine, options: Options, flags: Serve) -> ExitCode {
    use talos::server::{DEFAULT_ADDR, Server, Settings};
    let addr = std::net::SocketAddr::new(
        flags.bind.unwrap_or(DEFAULT_ADDR.ip()),
        flags.port.unwrap_or(DEFAULT_ADDR.port()),
    );
    let mut settings = Settings::new(addr);
    settings.default_lang = options.lang;
    settings.token = flags
        .token
        .or_else(|| std::env::var("TALOS_TOKEN").ok().filter(|t| !t.is_empty()));
    let server = match Server::bind(engine, settings) {
        Ok(server) => server,
        Err(error @ talos::server::StartError::Bind(..)) => {
            eprintln!("talos: {error}");
            return ExitCode::from(EXIT_IO);
        }
        Err(error) => {
            eprintln!("talos: {error}");
            return ExitCode::from(EXIT_USAGE);
        }
    };
    match server.local_addr() {
        Ok(addr) => eprintln!("talos {VERSION}: serving on http://{addr}"),
        Err(error) => {
            eprintln!("talos: {error}");
            return ExitCode::from(EXIT_IO);
        }
    }
    match server.run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("talos: {error}");
            ExitCode::from(EXIT_IO)
        }
    }
}

/// `talos serve` in a build without the `server` feature.
/// `talos packs …`: prints the report; fails when a pack is refused or
/// has errors.
#[cfg(feature = "knowledge")]
fn packs(command: &Packs) -> ExitCode {
    let (report, ok) = match command {
        Packs::List => talos::packs::list(),
        Packs::Info(path) => talos::packs::info(path),
        Packs::Check(path) => talos::packs::check(path),
    };
    print!("{report}");
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

#[cfg(not(feature = "knowledge"))]
fn packs(_command: &Packs) -> ExitCode {
    eprintln!("talos: this build has no knowledge store; rebuild with `--features knowledge`");
    ExitCode::from(EXIT_USAGE)
}

#[cfg(not(feature = "server"))]
fn serve(_engine: Engine, _options: Options, _flags: Serve) -> ExitCode {
    eprintln!("talos: this build has no server; rebuild with `--features server`");
    ExitCode::from(EXIT_USAGE)
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
        Command::Packs(command) => return packs(command),
        Command::Ask(options, _)
        | Command::Explain(options, _)
        | Command::Chat(options)
        | Command::Eval(options, _)
        | Command::Serve(options, _) => *options,
        Command::SkillsList => Options::default(),
    };
    let config = Config {
        seed: options.seed,
        ..Config::default()
    };
    let mut engine = match talos_skills::engine(config) {
        Ok(engine) => engine,
        Err(error) => {
            eprintln!("talos: {error}");
            return ExitCode::from(EXIT_SOFTWARE);
        }
    };
    // What the user asked to be remembered, and the packs: the turn
    // commands read them; `eval` and `skills list` never touch them, so a
    // battery's score does not depend on who ran it.
    if matches!(
        command,
        Command::Ask(..) | Command::Explain(..) | Command::Chat(_) | Command::Serve(..)
    ) {
        if let Some(warning) = talos::data::attach_user_store(&mut engine) {
            eprintln!("talos: {warning}");
        }
        #[cfg(feature = "knowledge")]
        for warning in talos::data::attach_knowledge(&mut engine) {
            eprintln!("talos: {warning}");
        }
    }
    match command {
        Command::Ask(options, text) => one_shot(&engine, options, text, false),
        Command::Explain(options, text) => one_shot(&engine, options, text, true),
        Command::SkillsList => {
            skills_list(&engine);
            ExitCode::SUCCESS
        }
        Command::Eval(options, dir) => run_eval(&engine, options, &dir),
        Command::Serve(options, flags) => serve(engine, options, flags),
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
        assert_eq!(
            parse(&["serve"]),
            Ok(Command::Serve(Options::default(), Serve::default()))
        );
        assert_eq!(
            parse(&["serve", "--bind", "0.0.0.0", "--port=0", "--token", "t"]),
            Ok(Command::Serve(
                Options::default(),
                Serve {
                    bind: Some([0, 0, 0, 0].into()),
                    port: Some(0),
                    token: Some("t".to_owned()),
                }
            ))
        );
        assert!(parse(&["serve", "--bind", "nowhere"]).is_err());
        assert!(parse(&["serve", "--port", "70000"]).is_err());
        assert!(parse(&["serve", "text"]).is_err());
        assert!(parse(&["ask", "--port", "1", "hi"]).is_err());
        assert_eq!(
            parse(&["eval"]),
            Ok(Command::Eval(Options::default(), PathBuf::from("evals")))
        );
        assert_eq!(
            parse(&["eval", "--dir", "x", "--json"]),
            Ok(Command::Eval(
                Options {
                    json: true,
                    ..Options::default()
                },
                PathBuf::from("x")
            ))
        );
        assert!(parse(&["eval", "text"]).is_err());
        assert!(parse(&["ask", "--dir", "x", "hi"]).is_err());
    }

    #[test]
    fn exit_statuses_follow_the_api() {
        assert_eq!(exit_status(Status::Answered), 0);
        assert_eq!(exit_status(Status::Deflected), 2);
        assert_eq!(exit_status(Status::Unhandled), 3);
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
