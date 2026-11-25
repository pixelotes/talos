//! tiny_http with its default settings.
fn main() {
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    println!("{}", server.server_addr());
    for mut request in server.incoming_requests() {
        let mut body = String::new();
        let _ = request.as_reader().read_to_string(&mut body);
        let header = tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap();
        let _ =
            request.respond(tiny_http::Response::from_string(http_stack::BODY).with_header(header));
    }
}
