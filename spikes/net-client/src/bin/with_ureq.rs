//! The GET through `ureq` with rustls (ring, the webpki root set).
use std::time::Duration;

fn main() {
    let url = std::env::args().nth(1).expect("a URL");
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(5)))
        .user_agent("talos-spike/0.1")
        .build()
        .into();
    let body = agent
        .get(&url)
        .call()
        .expect("the request succeeds")
        .body_mut()
        .read_to_string()
        .expect("a text body");
    println!("{} bytes", body.len());
}
