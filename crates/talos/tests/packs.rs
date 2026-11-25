//! The binary with a data directory of its own (never the user's): `user.toml`
//! imported into `user.db` once, packs attached or left out with a reason,
//! `talos packs`, and `/health` listing them.

#![cfg(feature = "knowledge")]
#![allow(clippy::unwrap_used, clippy::expect_used, reason = "tests")]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use talos_core::knowledge::build::build_file;

const SEED: &str = include_str!("../../../knowledge/seed.toml");

fn data_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("talos-packs-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("packs")).unwrap();
    dir
}

fn talos(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_talos"))
        .args(args)
        .env("TALOS_DATA_DIR", dir)
        .output()
        .unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn user_toml_moves_into_user_db_once_and_an_ordinary_turn_writes_nothing() {
    let dir = data_dir("migrate");
    let toml = "version = 1\n\n[preferences]\nname = \"Ana\"\n";
    std::fs::write(dir.join("user.toml"), toml).unwrap();
    let out = talos(&dir, &["ask", "¿cómo me llamo?"]);
    assert!(text(&out.stdout).contains("Ana"), "{}", text(&out.stdout));
    assert!(dir.join("user.db").exists());
    assert!(!dir.join("user.toml").exists());
    assert_eq!(
        std::fs::read_to_string(dir.join("user.toml.migrated")).unwrap(),
        toml
    );
    let before = std::fs::read(dir.join("user.db")).unwrap();
    let out = talos(&dir, &["ask", "¿cómo me llamo?"]);
    assert!(text(&out.stdout).contains("Ana"), "{}", text(&out.stdout));
    assert_eq!(std::fs::read(dir.join("user.db")).unwrap(), before);
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["packs", "user.db", "user.toml.migrated"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_fresh_data_directory_gets_nothing_written_by_a_question() {
    let dir = data_dir("fresh");
    let out = talos(&dir, &["ask", "hola"]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "only packs/");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn packs_are_listed_checked_and_refused_with_a_reason() {
    let dir = data_dir("list");
    let pack = dir.join("packs/core.talospack");
    build_file(SEED, &pack).unwrap();
    let out = talos(&dir, &["packs", "list"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stdout));
    assert!(
        text(&out.stdout).starts_with("core "),
        "{}",
        text(&out.stdout)
    );

    let out = talos(&dir, &["packs", "check", pack.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stdout));
    assert!(text(&out.stdout).trim_end().ends_with("core 1.0 ok"));
    let out = talos(&dir, &["packs", "info", pack.to_str().unwrap()]);
    let info = text(&out.stdout);
    assert!(info.contains("concepts    63"), "{info}");
    assert!(info.contains("claims      55"), "{info}");

    let junk = dir.join("packs/junk.talospack");
    std::fs::write(&junk, b"not a pack").unwrap();
    let out = talos(&dir, &["packs", "list"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        text(&out.stdout).contains("refused"),
        "{}",
        text(&out.stdout)
    );
    let out = talos(&dir, &["packs", "check", junk.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    // A turn still answers, and says what it left out.
    let out = talos(&dir, &["ask", "hola"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(
        text(&out.stderr).contains("junk.talospack left out"),
        "{}",
        text(&out.stderr)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn health_lists_the_attached_packs_and_the_ones_left_out() {
    let dir = data_dir("health");
    build_file(SEED, &dir.join("packs/core.talospack")).unwrap();
    std::fs::write(dir.join("packs/junk.talospack"), b"not a pack").unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_talos"))
        .args(["serve", "--port", "0"])
        .env_remove("TALOS_TOKEN")
        .env("TALOS_DATA_DIR", &dir)
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stderr = BufReader::new(child.stderr.take().unwrap());
    let mut line = String::new();
    // The warning about the junk pack comes first, then the address.
    while !line.contains("http://") {
        line.clear();
        stderr.read_line(&mut line).unwrap();
    }
    let addr = line.trim().rsplit("http://").next().unwrap().to_owned();
    let mut stream = TcpStream::connect(&addr).unwrap();
    stream
        .write_all(b"GET /health HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    child.kill().unwrap();
    child.wait().unwrap();
    let body = response.split("\r\n\r\n").nth(1).unwrap();
    let json: serde_json::Value = serde_json::from_str(body).unwrap();
    assert_eq!(json["packs"][0]["id"], "core");
    assert_eq!(
        json["packs"][0]["languages"],
        serde_json::json!(["es", "en"])
    );
    assert!(
        json["failures"][0]["pack"]
            .as_str()
            .unwrap()
            .ends_with("junk.talospack")
    );
    let _ = std::fs::remove_dir_all(&dir);
}
