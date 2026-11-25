//! The HTTP/1.1 wire: reading one request off a connection within limits,
//! writing one response (ADR 0017). Only what the three endpoints need:
//! `Content-Length` and chunked request bodies, `Expect: 100-continue`,
//! keep-alive, and every response with a `Content-Length`.

use std::io::{self, BufRead, Write};
use std::time::Instant;

use super::Limits;

/// One request, read in full.
#[derive(Debug)]
pub(crate) struct Request {
    pub(crate) method: String,
    /// The path, without the query string.
    pub(crate) path: String,
    /// Header names in lower case, in arrival order.
    pub(crate) headers: Vec<(String, String)>,
    pub(crate) body: Vec<u8>,
    /// Whether the connection should close after the response.
    pub(crate) close: bool,
}

impl Request {
    /// The first value of a header, by its lower-case name.
    pub(crate) fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }
}

/// Why no request could be read.
#[derive(Debug)]
pub(crate) enum ReadError {
    /// The peer closed, or went idle, between requests: close quietly.
    Gone,
    /// The request is malformed or over a limit: answer with this status
    /// and message, then close.
    Reject(u16, &'static str),
}

impl ReadError {
    fn bad(message: &'static str) -> Self {
        Self::Reject(400, message)
    }
}

/// One response, written whole with a `Content-Length`.
#[derive(Debug)]
pub(crate) struct Response {
    pub(crate) status: u16,
    pub(crate) content_type: &'static str,
    /// Extra headers.
    pub(crate) headers: Vec<(&'static str, String)>,
    pub(crate) body: Vec<u8>,
}

impl Response {
    pub(crate) fn json(status: u16, body: Vec<u8>) -> Self {
        Self {
            status,
            content_type: "application/json",
            headers: Vec::new(),
            body,
        }
    }

    pub(crate) fn with_header(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.headers.push((name, value.into()));
        self
    }
}

/// The reason phrase of each status the server sends.
fn reason(status: u16) -> &'static str {
    match status {
        100 => "Continue",
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        411 => "Length Required",
        413 => "Content Too Large",
        431 => "Request Header Fields Too Large",
        501 => "Not Implemented",
        503 => "Service Unavailable",
        505 => "HTTP Version Not Supported",
        _ => "Error",
    }
}

/// Writes a response in one write. `close` adds `Connection: close`.
pub(crate) fn write_response(
    out: &mut impl Write,
    response: &Response,
    close: bool,
) -> io::Result<()> {
    let mut head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\n",
        response.status,
        reason(response.status),
        response.content_type,
        response.body.len()
    );
    for (name, value) in &response.headers {
        head.push_str(name);
        head.push_str(": ");
        head.push_str(value);
        head.push_str("\r\n");
    }
    if close {
        head.push_str("Connection: close\r\n");
    }
    head.push_str("\r\n");
    let mut bytes = head.into_bytes();
    bytes.extend_from_slice(&response.body);
    out.write_all(&bytes)?;
    out.flush()
}

/// A read failure: a timeout or a reset.
fn io_error(error: &io::Error, started: bool) -> ReadError {
    let timed_out = matches!(
        error.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    );
    if started && timed_out {
        ReadError::Reject(408, "the request was not received in time")
    } else {
        ReadError::Gone
    }
}

/// Reads one line (up to and including `\n`) into `line`, which must be
/// empty, giving up past `limit` bytes or the deadline. `Ok(false)` at end
/// of input before any byte. `started` says whether part of a request has
/// arrived, which turns a timeout or a close into an error to answer.
fn read_line(
    input: &mut impl BufRead,
    line: &mut Vec<u8>,
    limit: usize,
    deadline: Instant,
    started: bool,
) -> Result<bool, ReadError> {
    loop {
        if Instant::now() > deadline {
            return Err(ReadError::Reject(
                408,
                "the request was not received in time",
            ));
        }
        let available = match input.fill_buf() {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(io_error(&e, started || !line.is_empty())),
        };
        if available.is_empty() {
            return if line.is_empty() && !started {
                Ok(false)
            } else {
                Err(ReadError::Gone)
            };
        }
        let (taken, done) = match available.iter().position(|&b| b == b'\n') {
            Some(end) => (end + 1, true),
            None => (available.len(), false),
        };
        if line.len() + taken > limit {
            return Err(ReadError::Reject(431, "the request head is too large"));
        }
        line.extend_from_slice(&available[..taken]);
        input.consume(taken);
        if done {
            return Ok(true);
        }
    }
}

/// Reads exactly `length` bytes onto `body`.
fn read_exact(
    input: &mut impl BufRead,
    body: &mut Vec<u8>,
    length: usize,
    deadline: Instant,
) -> Result<(), ReadError> {
    let mut left = length;
    while left > 0 {
        if Instant::now() > deadline {
            return Err(ReadError::Reject(
                408,
                "the request was not received in time",
            ));
        }
        let available = match input.fill_buf() {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(io_error(&e, true)),
        };
        if available.is_empty() {
            return Err(ReadError::Gone);
        }
        let taken = available.len().min(left);
        body.extend_from_slice(&available[..taken]);
        input.consume(taken);
        left -= taken;
    }
    Ok(())
}

/// A line without its `\r\n` or `\n`, as text.
fn text(line: &[u8]) -> Result<&str, ReadError> {
    let line = line.strip_suffix(b"\n").unwrap_or(line);
    let line = line.strip_suffix(b"\r").unwrap_or(line);
    std::str::from_utf8(line).map_err(|_| ReadError::bad("the request head is not valid text"))
}

/// How the body is framed.
enum Framing {
    None,
    Length(usize),
    Chunked,
}

/// Reads one request. `out` receives the interim `100 Continue` when the
/// client asks for it.
pub(crate) fn read_request(
    input: &mut impl BufRead,
    out: &mut impl Write,
    limits: &Limits,
) -> Result<Request, ReadError> {
    let mut line = Vec::new();
    // Before the first byte, the connection's read timeout decides how long
    // an idle keep-alive connection is held; the deadline starts with it.
    let mut deadline = Instant::now() + limits.request_deadline + limits.read_timeout;
    let mut head_bytes = 0;
    // A client may send blank lines before a request (RFC 9112 2.2).
    loop {
        line.clear();
        if !read_line(input, &mut line, limits.max_head, deadline, false)? {
            return Err(ReadError::Gone);
        }
        if line != b"\r\n" && line != b"\n" {
            break;
        }
    }
    deadline = Instant::now() + limits.request_deadline;
    head_bytes += line.len();
    let request_line = text(&line)?.to_owned();
    let mut parts = request_line.split(' ');
    let (Some(method), Some(target), Some(version), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(ReadError::bad("malformed request line"));
    };
    if method.is_empty() || !method.bytes().all(|b| b.is_ascii_uppercase()) {
        return Err(ReadError::bad("malformed request method"));
    }
    let close_by_default = match version {
        "HTTP/1.1" => false,
        "HTTP/1.0" => true,
        v if v.starts_with("HTTP/") => {
            return Err(ReadError::Reject(
                505,
                "only HTTP/1.1 and HTTP/1.0 are served",
            ));
        }
        _ => return Err(ReadError::bad("malformed request line")),
    };
    let path = target
        .split(['?', '#'])
        .next()
        .unwrap_or_default()
        .to_owned();
    if !path.starts_with('/') {
        return Err(ReadError::bad("the request target must be a path"));
    }

    let mut headers = Vec::new();
    loop {
        line.clear();
        let room = limits.max_head.saturating_sub(head_bytes);
        read_line(input, &mut line, room, deadline, true)?;
        head_bytes += line.len();
        let header = text(&line)?;
        if header.is_empty() {
            break;
        }
        if headers.len() >= limits.max_headers {
            return Err(ReadError::Reject(431, "too many request headers"));
        }
        let Some((name, value)) = header.split_once(':') else {
            return Err(ReadError::bad("malformed header line"));
        };
        if name.is_empty() || name.bytes().any(|b| b.is_ascii_whitespace()) {
            return Err(ReadError::bad("malformed header name"));
        }
        headers.push((name.to_ascii_lowercase(), value.trim().to_owned()));
    }

    let has_token = |name: &str, token: &str| {
        headers
            .iter()
            .filter(|(n, _)| n == name)
            .flat_map(|(_, v)| v.split(','))
            .any(|t| t.trim().eq_ignore_ascii_case(token))
    };
    let close = close_by_default || has_token("connection", "close");
    let framing = framing(&headers, limits)?;
    let mut body = Vec::new();
    if !matches!(framing, Framing::None) && has_token("expect", "100-continue") {
        let interim = b"HTTP/1.1 100 Continue\r\n\r\n";
        if out.write_all(interim).and_then(|()| out.flush()).is_err() {
            return Err(ReadError::Gone);
        }
    }
    match framing {
        Framing::None => {}
        Framing::Length(length) => read_exact(input, &mut body, length, deadline)?,
        Framing::Chunked => read_chunked(input, &mut body, limits, deadline)?,
    }
    Ok(Request {
        method: method.to_owned(),
        path,
        headers,
        body,
        close,
    })
}

/// The body's framing from the headers (RFC 9112 6.3).
fn framing(headers: &[(String, String)], limits: &Limits) -> Result<Framing, ReadError> {
    let mut encodings = headers
        .iter()
        .filter(|(n, _)| n == "transfer-encoding")
        .flat_map(|(_, v)| v.split(','))
        .map(str::trim)
        .filter(|t| !t.is_empty());
    if let Some(first) = encodings.next() {
        // Only plain `chunked` is understood; anything else cannot be read.
        if !first.eq_ignore_ascii_case("chunked") || encodings.next().is_some() {
            return Err(ReadError::Reject(501, "unsupported transfer encoding"));
        }
        if headers.iter().any(|(n, _)| n == "content-length") {
            return Err(ReadError::bad(
                "both Content-Length and Transfer-Encoding were sent",
            ));
        }
        return Ok(Framing::Chunked);
    }
    let mut length = None;
    for (_, value) in headers.iter().filter(|(n, _)| n == "content-length") {
        for item in value.split(',') {
            let item = item.trim();
            if item.is_empty() || !item.bytes().all(|b| b.is_ascii_digit()) {
                return Err(ReadError::bad("malformed Content-Length"));
            }
            let n: usize = item
                .parse()
                .map_err(|_| ReadError::Reject(413, "the request body is too large"))?;
            if length.is_some_and(|l| l != n) {
                return Err(ReadError::bad("conflicting Content-Length values"));
            }
            length = Some(n);
        }
    }
    match length {
        Some(n) if n > limits.max_body => {
            Err(ReadError::Reject(413, "the request body is too large"))
        }
        Some(0) | None => Ok(Framing::None),
        Some(n) => Ok(Framing::Length(n)),
    }
}

/// Reads a chunked body, its trailers discarded.
fn read_chunked(
    input: &mut impl BufRead,
    body: &mut Vec<u8>,
    limits: &Limits,
    deadline: Instant,
) -> Result<(), ReadError> {
    let mut line = Vec::new();
    loop {
        line.clear();
        read_line(input, &mut line, 1_024, deadline, true)?;
        let size_text = text(&line)?;
        let size_text = size_text.split(';').next().unwrap_or_default().trim();
        let size = usize::from_str_radix(size_text, 16)
            .map_err(|_| ReadError::bad("malformed chunk size"))?;
        if size == 0 {
            break;
        }
        if body.len().saturating_add(size) > limits.max_body {
            return Err(ReadError::Reject(413, "the request body is too large"));
        }
        read_exact(input, body, size, deadline)?;
        line.clear();
        read_line(input, &mut line, 2, deadline, true)?;
        if !text(&line)?.is_empty() {
            return Err(ReadError::bad("malformed chunk"));
        }
    }
    // Trailer fields, up to the blank line.
    let mut trailer_bytes = 0;
    loop {
        line.clear();
        let room = limits.max_head.saturating_sub(trailer_bytes);
        read_line(input, &mut line, room, deadline, true)?;
        trailer_bytes += line.len();
        if text(&line)?.is_empty() {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(raw: &str) -> Result<Request, ReadError> {
        let mut out = Vec::new();
        read_request(&mut raw.as_bytes(), &mut out, &Limits::default())
    }

    fn rejected(raw: &str) -> u16 {
        match read(raw) {
            Err(ReadError::Reject(status, _)) => status,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn reads_a_request_with_a_body() {
        let r =
            read("\r\nPOST /v1/x?a=1 HTTP/1.1\r\nHost: h\r\nContent-Length: 2\r\n\r\n{}").unwrap();
        assert_eq!(r.method, "POST");
        assert_eq!(r.path, "/v1/x");
        assert_eq!(r.header("host"), Some("h"));
        assert_eq!(r.body, b"{}");
        assert!(!r.close);
        let r = read("GET / HTTP/1.0\n\n").unwrap();
        assert!(r.close);
        let r = read("GET / HTTP/1.1\r\nConnection: keep-alive, Close\r\n\r\n").unwrap();
        assert!(r.close);
    }

    #[test]
    fn reads_a_chunked_body() {
        let r = read(
            "POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n2;x=y\r\n{\"\r\n3\r\na\":\r\n0\r\nT: 1\r\n\r\n",
        )
        .unwrap();
        assert_eq!(r.body, b"{\"a\":");
    }

    #[test]
    fn rejects_what_it_cannot_read() {
        assert_eq!(rejected("GET /\r\n\r\n"), 400);
        assert_eq!(rejected("GET / HTTP/2.0\r\n\r\n"), 505);
        assert_eq!(rejected("get / HTTP/1.1\r\n\r\n"), 400);
        assert_eq!(rejected("GET / HTTP/1.1\r\nbad header\r\n\r\n"), 400);
        assert_eq!(
            rejected("POST / HTTP/1.1\r\nContent-Length: 1\r\nContent-Length: 2\r\n\r\n"),
            400
        );
        assert_eq!(
            rejected("POST / HTTP/1.1\r\nContent-Length: 99999999999\r\n\r\n"),
            413
        );
        assert_eq!(
            rejected("POST / HTTP/1.1\r\nTransfer-Encoding: gzip\r\n\r\n"),
            501
        );
        let long = format!("GET /{} HTTP/1.1\r\n\r\n", "a".repeat(20_000));
        assert_eq!(rejected(&long), 431);
        let many = format!("GET / HTTP/1.1\r\n{}\r\n", "A: b\r\n".repeat(200));
        assert_eq!(rejected(&many), 431);
        // A body cut short is a closed connection, not a request.
        assert!(matches!(
            read("POST / HTTP/1.1\r\nContent-Length: 5\r\n\r\n{}"),
            Err(ReadError::Gone)
        ));
        assert!(matches!(read(""), Err(ReadError::Gone)));
    }

    #[test]
    fn answers_expect_continue() {
        let mut out = Vec::new();
        let raw = "POST / HTTP/1.1\r\nExpect: 100-continue\r\nContent-Length: 2\r\n\r\n{}";
        read_request(&mut raw.as_bytes(), &mut out, &Limits::default()).unwrap();
        assert_eq!(out, b"HTTP/1.1 100 Continue\r\n\r\n");
    }

    #[test]
    fn writes_a_response() {
        let mut out = Vec::new();
        let response =
            Response::json(200, b"{}".to_vec()).with_header("X-Talos-Status", "answered");
        write_response(&mut out, &response, true).unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\
             X-Talos-Status: answered\r\nConnection: close\r\n\r\n{}"
        );
    }
}
