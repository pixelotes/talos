//! `talos serve`: the OpenAI-compatible HTTP server of docs/API.md section 2
//! (ADR 0010), on a small HTTP/1.1 implementation over `std` (ADR 0017).
//!
//! One thread per connection, up to [`Limits::max_connections`]; past that a
//! connection is answered `503` and closed. Every read and write has a
//! timeout and a request has a deadline as a whole, so a slow or half-open
//! client holds only its own thread, and only for a bounded time. The engine
//! is shared by every connection; it serialises turns of one conversation
//! and runs turns of different ones in parallel.

mod api;
mod http;
mod identity;

use std::fmt;
use std::io::{self, BufReader};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

use talos_core::{Engine, Lang, Timestamp};

use crate::clock::LocalClock;
use identity::Transcripts;

/// The default address: loopback only (DESIGN.md section 11).
pub const DEFAULT_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8099);

/// Where the current instant comes from: the system clock in `talos serve`,
/// a fixed one in tests.
pub type Clock = Arc<dyn Fn() -> Timestamp + Send + Sync>;

/// The bounds that keep one client from starving the others.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// The largest request body, in bytes.
    pub max_body: usize,
    /// The largest request line plus headers, in bytes.
    pub max_head: usize,
    /// The most header lines in a request.
    pub max_headers: usize,
    /// The most connections served at once.
    pub max_connections: usize,
    /// How long a read may wait; also how long an idle keep-alive
    /// connection is kept.
    pub read_timeout: Duration,
    /// How long a write may wait.
    pub write_timeout: Duration,
    /// How long a whole request may take to arrive, once it has started.
    pub request_deadline: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_body: 1 << 20,
            max_head: 16 << 10,
            max_headers: 100,
            max_connections: 64,
            read_timeout: Duration::from_secs(10),
            write_timeout: Duration::from_secs(10),
            request_deadline: Duration::from_secs(30),
        }
    }
}

/// How the server is run.
#[derive(Clone)]
pub struct Settings {
    /// The address to listen on; port 0 picks a free one.
    pub addr: SocketAddr,
    /// The bearer token every request must carry. Required to listen on
    /// anything but a loopback address.
    pub token: Option<String>,
    /// The language hint for requests that give none (`--lang`).
    pub default_lang: Option<Lang>,
    /// Size limits and timeouts.
    pub limits: Limits,
    /// The clock read before each turn.
    pub clock: Clock,
    /// Whether `elapsed_us` is measured; with it off it is always 0, so
    /// that responses are byte-identical from run to run.
    pub stopwatch: bool,
}

impl fmt::Debug for Settings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Settings")
            .field("addr", &self.addr)
            .field("token", &self.token.as_ref().map(|_| "…"))
            .field("default_lang", &self.default_lang)
            .field("limits", &self.limits)
            .field("stopwatch", &self.stopwatch)
            .finish_non_exhaustive()
    }
}

impl Settings {
    /// Settings for `addr` with the system clock, no token and the default
    /// limits.
    #[must_use]
    pub fn new(addr: SocketAddr) -> Self {
        let clock = LocalClock::default();
        Self {
            addr,
            token: None,
            default_lang: None,
            limits: Limits::default(),
            clock: Arc::new(move || clock.now()),
            stopwatch: true,
        }
    }
}

/// Why the server did not start.
#[derive(Debug)]
pub enum StartError {
    /// A non-loopback address was asked for without a token.
    TokenRequired(SocketAddr),
    /// The token is empty or not printable ASCII.
    BadToken,
    /// The address could not be bound.
    Bind(SocketAddr, io::Error),
}

impl fmt::Display for StartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TokenRequired(addr) => write!(
                f,
                "refusing to listen on {addr} without a token: anything but a loopback \
                 address needs one (set TALOS_TOKEN or pass --token)"
            ),
            Self::BadToken => write!(f, "the token must be non-empty printable ASCII"),
            Self::Bind(addr, error) => write!(f, "cannot listen on {addr}: {error}"),
        }
    }
}

impl std::error::Error for StartError {}

/// What every connection shares.
struct Shared {
    engine: Engine,
    token: Option<String>,
    default_lang: Option<Lang>,
    limits: Limits,
    clock: Clock,
    stopwatch: bool,
    transcripts: Transcripts,
    /// Completions answered, for their ids.
    completions: AtomicU64,
    /// Connections being served.
    active: AtomicUsize,
}

/// A bound server, not yet accepting.
pub struct Server {
    listener: TcpListener,
    shared: Arc<Shared>,
}

impl fmt::Debug for Server {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Server")
            .field("addr", &self.listener.local_addr().ok())
            .finish_non_exhaustive()
    }
}

impl Server {
    /// Binds the address. The fingerprint table has the bounds of the
    /// engine's dialog state (`max_conversations`, `idle_expiry_seconds`).
    ///
    /// # Errors
    ///
    /// [`StartError::TokenRequired`] for a non-loopback address without a
    /// token, [`StartError::BadToken`] for a token that cannot travel in a
    /// header, [`StartError::Bind`] if the address cannot be bound.
    pub fn bind(engine: Engine, settings: Settings) -> Result<Self, StartError> {
        if let Some(token) = &settings.token
            && (token.is_empty() || !token.bytes().all(|b| b.is_ascii_graphic()))
        {
            return Err(StartError::BadToken);
        }
        if settings.token.is_none() && !settings.addr.ip().is_loopback() {
            return Err(StartError::TokenRequired(settings.addr));
        }
        let listener =
            TcpListener::bind(settings.addr).map_err(|e| StartError::Bind(settings.addr, e))?;
        let config = engine.config();
        // Read the clock once now, so the first request does not pay for
        // looking up the local offset.
        let _ = (settings.clock)();
        let shared = Shared {
            transcripts: Transcripts::new(config.max_conversations, config.idle_expiry_seconds),
            engine,
            token: settings.token,
            default_lang: settings.default_lang,
            limits: settings.limits,
            clock: settings.clock,
            stopwatch: settings.stopwatch,
            completions: AtomicU64::new(0),
            active: AtomicUsize::new(0),
        };
        Ok(Self {
            listener,
            shared: Arc::new(shared),
        })
    }

    /// The address actually bound (with the port chosen, for port 0).
    ///
    /// # Errors
    ///
    /// As [`TcpListener::local_addr`].
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    /// Accepts connections until the process ends.
    ///
    /// # Errors
    ///
    /// Only if the listener itself fails; a failed connection is dropped.
    pub fn run(self) -> io::Result<()> {
        loop {
            let stream = match self.listener.accept() {
                Ok((stream, _)) => stream,
                // A connection reset before it was accepted, or too many
                // open files for a moment: keep listening.
                Err(e) if is_transient(&e) => {
                    if out_of_files(&e) {
                        std::thread::sleep(Duration::from_millis(50));
                    }
                    continue;
                }
                Err(e) => return Err(e),
            };
            let shared = Arc::clone(&self.shared);
            let limits = shared.limits;
            if shared.active.fetch_add(1, Ordering::AcqRel) >= limits.max_connections {
                shared.active.fetch_sub(1, Ordering::AcqRel);
                refuse(stream, &limits);
                continue;
            }
            let spawned = std::thread::Builder::new()
                .name("talos-conn".to_owned())
                .spawn(move || {
                    serve_connection(&shared, stream);
                    shared.active.fetch_sub(1, Ordering::AcqRel);
                });
            if spawned.is_err() {
                self.shared.active.fetch_sub(1, Ordering::AcqRel);
            }
        }
    }
}

fn is_transient(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::ConnectionAborted
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::Interrupted
            | io::ErrorKind::WouldBlock
    ) || out_of_files(error)
}

/// `ENFILE` or `EMFILE` (the same numbers on Linux and the BSDs).
fn out_of_files(error: &io::Error) -> bool {
    error
        .raw_os_error()
        .is_some_and(|code| code == 23 || code == 24)
}

/// Answers `503` to a connection over the limit, without reading it.
fn refuse(mut stream: TcpStream, limits: &Limits) {
    let _ = stream.set_write_timeout(Some(limits.write_timeout.min(Duration::from_secs(1))));
    let error = api::ApiError::new(503, "too many connections; try again shortly");
    let _ = http::write_response(&mut stream, &error.response(), true);
}

/// After an error response to a request that was not read in full: stops
/// sending, then reads and discards what the client is still sending, for
/// a short while, before closing. Closing with unread input makes the
/// system reset the connection, and a reset can destroy the error response
/// before the client reads it (RFC 9112 9.6).
fn linger(input: &mut BufReader<TcpStream>, output: &TcpStream) {
    const MAX_BYTES: usize = 1 << 20;
    const MAX_TIME: Duration = Duration::from_secs(1);
    if output.shutdown(std::net::Shutdown::Write).is_err()
        || output.set_read_timeout(Some(MAX_TIME)).is_err()
    {
        return;
    }
    let deadline = std::time::Instant::now() + MAX_TIME;
    let mut discarded = 0;
    let mut buffer = [0; 8 << 10];
    while discarded < MAX_BYTES && std::time::Instant::now() < deadline {
        match io::Read::read(input, &mut buffer) {
            Ok(0) | Err(_) => return,
            Ok(n) => discarded += n,
        }
    }
}

/// Serves requests on one connection until it closes, errs or idles out.
fn serve_connection(shared: &Shared, stream: TcpStream) {
    let limits = shared.limits;
    let ready = stream
        .set_read_timeout(Some(limits.read_timeout))
        .and_then(|()| stream.set_write_timeout(Some(limits.write_timeout)))
        .and_then(|()| stream.set_nodelay(true));
    let Ok(mut output) = ready.and_then(|()| stream.try_clone()) else {
        return;
    };
    let mut input = BufReader::new(stream);
    loop {
        let request = match http::read_request(&mut input, &mut output, &limits) {
            Ok(request) => request,
            Err(http::ReadError::Gone) => return,
            Err(http::ReadError::Reject(status, message)) => {
                let error = api::ApiError::new(status, message);
                if http::write_response(&mut output, &error.response(), true).is_ok() {
                    linger(&mut input, &output);
                }
                return;
            }
        };
        let response = api::handle(shared, &request);
        if http::write_response(&mut output, &response, request.close).is_err() || request.close {
            return;
        }
    }
}
