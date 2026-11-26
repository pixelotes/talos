//! The network, for the front ends that are allowed it (`--net`, or
//! `TALOS_NET=1`): an HTTPS client, behind the `net` feature. The engine
//! itself links none of this; it is handed a [`Fetch`].
//!
//! A request has five seconds in all and its body at most a megabyte, so
//! no turn hangs on a slow server or fills memory from a large one. It is
//! sent as `talos/<version>`, with nothing about the machine or its owner
//! unless the owner gives a contact (`TALOS_CONTACT`: an address or a page),
//! which is then added in brackets. Sites that publish a policy for
//! automated clients ask for one, and limit or refuse clients without it
//! (Wikipedia answers 429 after a few requests).

use std::time::Duration;

use talos_core::net::{Fetch, FetchError};

/// The whole of a request: connecting, sending and reading the answer.
const TIMEOUT: Duration = Duration::from_secs(5);
/// The largest body read.
const LIMIT: u64 = 1 << 20;

/// The HTTPS client: `ureq` over rustls, with the Mozilla root set
/// compiled in, so that a static binary needs nothing from the system.
#[derive(Debug)]
pub struct Client {
    agent: ureq::Agent,
}

impl Client {
    /// A client that names `contact` to the sites it asks, when given; no
    /// connection is opened until the first request.
    #[must_use]
    pub fn new(contact: Option<&str>) -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(TIMEOUT))
            .user_agent(user_agent(contact))
            .https_only(true)
            .build()
            .into();
        Self { agent }
    }
}

/// `talos/<version>`, with the contact in brackets when there is one: its
/// printable ASCII without brackets, at most a hundred characters.
fn user_agent(contact: Option<&str>) -> String {
    let name = concat!("talos/", env!("CARGO_PKG_VERSION"));
    let contact: String = contact
        .unwrap_or_default()
        .chars()
        .filter(|c| c.is_ascii_graphic() && !matches!(c, '(' | ')'))
        .take(100)
        .collect();
    if contact.is_empty() {
        name.to_owned()
    } else {
        format!("{name} ({contact})")
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new(None)
    }
}

/// A failed request, noted on standard error for whoever runs Talos: the
/// user is only told that the page could not be read.
#[allow(clippy::print_stderr, reason = "the front end's log")]
fn failed(error: &ureq::Error) -> FetchError {
    eprintln!("talos: network: {error}");
    FetchError::Failed(error.to_string())
}

impl Fetch for Client {
    fn get(&self, url: &str) -> Result<String, FetchError> {
        let mut response = match self.agent.get(url).call() {
            Ok(response) => response,
            Err(ureq::Error::StatusCode(404)) => return Err(FetchError::NotFound),
            Err(error) => return Err(failed(&error)),
        };
        response
            .body_mut()
            .with_config()
            .limit(LIMIT)
            .read_to_string()
            .map_err(|error| failed(&error))
    }
}

#[cfg(test)]
mod tests {
    use super::user_agent;

    #[test]
    fn the_contact_is_added_only_when_given_and_kept_plain() {
        let name = concat!("talos/", env!("CARGO_PKG_VERSION"));
        assert_eq!(user_agent(None), name);
        assert_eq!(user_agent(Some("  ")), name);
        assert_eq!(
            user_agent(Some("https://example.org/me (x)\n")),
            format!("{name} (https://example.org/mex)")
        );
    }
}
