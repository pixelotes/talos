//! Server overhead: sends chat completions over one keep-alive connection
//! and reports, per kind of turn, the round trip, the engine's own time
//! (`talos.elapsed_us`) and their difference, the server's overhead
//! (docs/DEVELOPMENT.md, "Server").
//!
//! ```text
//! cargo run --release -p talos --example serve_load -- [ADDR] [N]
//! ```
//!
//! With no address it starts a server in this process on a free port. Each
//! request is a fresh one-message transcript, so every turn opens a
//! conversation and the dialog-state store fills and evicts as it would
//! under many clients.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a measuring tool; a failure is a crash"
)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::Instant;

use serde_json::Value as Json;
use talos::server::{Server, Settings};
use talos_core::Config;

/// Round trips, engine times and overheads of one kind of turn.
type Samples = (&'static str, Vec<i64>, Vec<i64>, Vec<i64>);

fn percentile(sorted: &[i64], q: f64) -> i64 {
    sorted[((sorted.len() as f64 - 1.0) * q).round() as usize]
}

fn main() {
    let mut args = std::env::args().skip(1);
    let addr = if let Some(addr) = args.next().filter(|a| a != "-") {
        addr
    } else {
        let engine = talos_skills::engine(Config::default()).unwrap();
        let server =
            Server::bind(engine, Settings::new("127.0.0.1:0".parse().unwrap())).expect("bind");
        let addr = server.local_addr().unwrap().to_string();
        std::thread::spawn(move || server.run());
        addr
    };
    let n: usize = args.next().map_or(1_000, |n| n.parse().unwrap());
    let stream = TcpStream::connect(&addr).unwrap();
    stream.set_nodelay(true).unwrap();
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut writer = stream;
    let kinds = [
        ("rule", "hola"),
        ("rule", "what time is it"),
        ("reflector", "creo que el cine es aburrido"),
        ("reflector", "i think movies are boring"),
    ];
    let mut by_kind: Vec<Samples> = vec![
        ("rule", Vec::new(), Vec::new(), Vec::new()),
        ("reflector", Vec::new(), Vec::new(), Vec::new()),
    ];
    for i in 0..n {
        let (kind, text) = kinds[i % kinds.len()];
        let body = serde_json::json!({
            "model": "talos",
            "messages": [{ "role": "user", "content": text }],
        })
        .to_string();
        let request = format!(
            "POST /v1/chat/completions HTTP/1.1\r\nHost: {addr}\r\n\
             Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        let started = Instant::now();
        writer.write_all(request.as_bytes()).unwrap();
        let mut length = 0;
        let mut line = String::new();
        loop {
            line.clear();
            reader.read_line(&mut line).unwrap();
            if line == "\r\n" {
                break;
            }
            if let Some((name, value)) = line.split_once(':')
                && name.eq_ignore_ascii_case("content-length")
            {
                length = value.trim().parse().unwrap();
            }
        }
        let mut response = vec![0; length];
        reader.read_exact(&mut response).unwrap();
        let round_trip = started.elapsed().as_micros() as i64;
        let json: Json = serde_json::from_slice(&response).unwrap();
        let engine = json["talos"]["elapsed_us"].as_i64().unwrap();
        let entry = by_kind.iter_mut().find(|e| e.0 == kind).unwrap();
        entry.1.push(round_trip);
        entry.2.push(engine);
        entry.3.push(round_trip - engine);
    }
    println!("{n} requests to {addr} over one connection (microseconds)");
    for (kind, mut trip, mut engine, mut overhead) in by_kind {
        trip.sort_unstable();
        engine.sort_unstable();
        overhead.sort_unstable();
        println!(
            "{kind:<10} round trip p50 {:>5} p99 {:>5} | engine p50 {:>5} p99 {:>5} | overhead p50 {:>5} p99 {:>5}",
            percentile(&trip, 0.5),
            percentile(&trip, 0.99),
            percentile(&engine, 0.5),
            percentile(&engine, 0.99),
            percentile(&overhead, 0.5),
            percentile(&overhead, 0.99),
        );
    }
}
