//! `load ADDR N [BODY]`: N POSTs to /v1/chat/completions over one keep-alive
//! connection; prints p50/p99/max round trip in microseconds.
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let addr = &args[1];
    let n: usize = args[2].parse().unwrap();
    let body = args.get(3).cloned().unwrap_or_else(|| {
        r#"{"model":"talos","messages":[{"role":"user","content":"hola"}]}"#.to_owned()
    });
    let stream = TcpStream::connect(addr).unwrap();
    stream.set_nodelay(true).unwrap();
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut writer = stream;
    let mut times = Vec::with_capacity(n);
    let request = format!(
        "POST /v1/chat/completions HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    for _ in 0..n {
        let started = Instant::now();
        writer.write_all(request.as_bytes()).unwrap();
        let mut length = 0usize;
        let mut line = String::new();
        loop {
            line.clear();
            reader.read_line(&mut line).unwrap();
            if line == "\r\n" {
                break;
            }
            if let Some((k, v)) = line.split_once(':') {
                if k.eq_ignore_ascii_case("content-length") {
                    length = v.trim().parse().unwrap();
                }
            }
        }
        let mut buf = vec![0; length];
        reader.read_exact(&mut buf).unwrap();
        times.push(started.elapsed().as_micros());
    }
    times.sort_unstable();
    let p = |q: f64| times[((times.len() as f64 - 1.0) * q).round() as usize];
    println!(
        "n={n} p50={}us p99={}us max={}us",
        p(0.5),
        p(0.99),
        times[n - 1]
    );
}
