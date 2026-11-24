//! The `talos` binary: the single entry point for the chat, ask and serve
//! front ends (DESIGN.md section 2, docs/API.md section 1). Each front end is
//! a thin shell around `Engine::respond`.

use std::io::{self, BufRead, IsTerminal, Write};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use talos_core::{Config, ConvId, Engine, Lang, Status, Timestamp, Turn};

const VERSION: &str = env!("CARGO_PKG_VERSION");

const USAGE: &str = "\
usage: talos [--lang es|en] ask <text>   answer one message, then exit
       talos [--lang es|en] chat         a conversation on stdin/stdout
       talos --version

exit status of `ask`: 0 answered, 2 deflected, 3 unhandled";

/// Wrong arguments (BSD `EX_USAGE`).
const EXIT_USAGE: u8 = 64;
/// The engine could not be built (BSD `EX_SOFTWARE`).
const EXIT_SOFTWARE: u8 = 70;
/// Reading or writing the terminal failed (BSD `EX_IOERR`).
const EXIT_IO: u8 = 74;

/// What the command line asked for.
#[derive(Debug, PartialEq, Eq)]
enum Command {
    Version,
    Help,
    Ask { lang: Option<Lang>, text: String },
    Chat { lang: Option<Lang> },
}

/// Parses the arguments after the program name. Flags may come before or
/// after the subcommand; `--` ends flags.
fn parse_args(args: &[String]) -> Result<Command, String> {
    let mut lang = None;
    let mut positional: Vec<&str> = Vec::new();
    let mut iter = args.iter();
    let mut flags_done = false;
    while let Some(arg) = iter.next() {
        let arg = arg.as_str();
        if flags_done || !arg.starts_with("--") && !matches!(arg, "-V" | "-h") {
            positional.push(arg);
            continue;
        }
        match arg {
            "--" => flags_done = true,
            "--version" | "-V" => return Ok(Command::Version),
            "--help" | "-h" => return Ok(Command::Help),
            "--lang" => {
                let value = iter.next().ok_or("`--lang` needs a value: es or en")?;
                lang = Some(value.parse::<Lang>().map_err(|e| e.to_string())?);
            }
            _ => match arg.strip_prefix("--lang=") {
                Some(value) => lang = Some(value.parse::<Lang>().map_err(|e| e.to_string())?),
                None => return Err(format!("unknown option `{arg}`")),
            },
        }
    }
    match positional.split_first() {
        Some((&"ask", words)) if !words.is_empty() => Ok(Command::Ask {
            lang,
            text: words.join(" "),
        }),
        Some((&"ask", _)) => Err("`ask` needs the text to answer".to_owned()),
        Some((&"chat", [])) => Ok(Command::Chat { lang }),
        Some((&"chat", _)) => Err("`chat` takes no text; type it after it starts".to_owned()),
        Some((other, _)) => Err(format!("unknown command `{other}`")),
        None => Ok(Command::Help),
    }
}

/// The current instant, read once per turn here in the front end; the engine
/// never reads the clock. The local UTC offset is not known without a
/// date/time crate (a Slice C decision), so it is left unset.
fn now() -> Timestamp {
    let seconds = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(after) => i64::try_from(after.as_secs()).unwrap_or(i64::MAX),
        Err(before) => i64::try_from(before.duration().as_secs()).map_or(i64::MIN, |s| -s),
    };
    Timestamp::from_unix(seconds)
}

/// The exit status of `talos ask` for an outcome.
fn exit_status(status: Status) -> u8 {
    match status {
        Status::Answered => 0,
        Status::Deflected => 2,
        Status::Unhandled => 3,
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
    let mut number = 0;
    if prompt {
        write!(output, "> ")?;
        output.flush()?;
    }
    for line in input.lines() {
        let line = line?;
        if !line.trim().is_empty() {
            number += 1;
            let turn = Turn::new(line, conversation.clone(), number, clock()).with_lang(lang);
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
    let lang = match command {
        Command::Version => {
            println!("talos {VERSION}");
            return ExitCode::SUCCESS;
        }
        Command::Help => {
            println!("talos {VERSION}\n\n{USAGE}");
            return ExitCode::SUCCESS;
        }
        Command::Ask { lang, .. } | Command::Chat { lang } => lang,
    };
    let engine = match talos_skills::engine(Config::default()) {
        Ok(engine) => engine,
        Err(error) => {
            eprintln!("talos: {error}");
            return ExitCode::from(EXIT_SOFTWARE);
        }
    };
    if let Command::Ask { text, .. } = command {
        let turn = Turn::new(text, ConvId::new("ask"), 1, now()).with_lang(lang);
        let outcome = engine.respond(turn);
        println!("{}", outcome.reply);
        return ExitCode::from(exit_status(outcome.status));
    }
    let stdin = io::stdin();
    let prompt = stdin.is_terminal();
    match chat(
        &engine,
        lang,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    fn parse(list: &[&str]) -> Result<Command, String> {
        parse_args(&args(list))
    }

    #[test]
    fn version_and_help() {
        assert_eq!(parse(&["--version"]), Ok(Command::Version));
        assert_eq!(parse(&["-V"]), Ok(Command::Version));
        assert_eq!(parse(&["--help"]), Ok(Command::Help));
        assert_eq!(parse(&[]), Ok(Command::Help));
    }

    #[test]
    fn ask_with_lang_before_or_after_the_command() {
        let expected = Ok(Command::Ask {
            lang: Some(Lang::En),
            text: "hola".to_owned(),
        });
        assert_eq!(parse(&["--lang", "en", "ask", "hola"]), expected);
        assert_eq!(parse(&["ask", "--lang", "en", "hola"]), expected);
        assert_eq!(parse(&["ask", "--lang=en", "hola"]), expected);
        assert_eq!(
            parse(&["ask", "qué", "hora", "es"]),
            Ok(Command::Ask {
                lang: None,
                text: "qué hora es".to_owned()
            })
        );
        assert_eq!(
            parse(&["ask", "--", "--lang"]),
            Ok(Command::Ask {
                lang: None,
                text: "--lang".to_owned()
            })
        );
    }

    #[test]
    fn chat_and_errors() {
        assert_eq!(
            parse(&["chat", "--lang", "es"]),
            Ok(Command::Chat {
                lang: Some(Lang::Es)
            })
        );
        assert!(parse(&["ask"]).is_err());
        assert!(parse(&["chat", "hola"]).is_err());
        assert!(parse(&["ask", "--lang", "fr", "x"]).is_err());
        assert!(parse(&["ask", "--lang"]).is_err());
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
    fn chat_answers_each_line_and_skips_blank_ones() {
        let engine = talos_skills::engine(Config::default()).unwrap();
        let input = "hola\n\n  \nhello\nescríbeme un poema\n";
        let mut output = Vec::new();
        let clock = || Timestamp::from_unix(1_805_046_120);
        chat(&engine, None, clock, input.as_bytes(), &mut output, false).unwrap();
        assert_eq!(
            String::from_utf8(output).unwrap(),
            "¡Hola!\nHi!\nNo sé hacer eso.\n"
        );
    }
}
