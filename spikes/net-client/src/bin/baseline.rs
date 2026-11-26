//! Fetches nothing: what the other two are measured against.
fn main() {
    let url = std::env::args().nth(1).unwrap_or_default();
    println!("{} bytes", url.len());
}
