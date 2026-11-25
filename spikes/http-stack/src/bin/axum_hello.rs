//! axum on a single-threaded tokio runtime, as DESIGN.md s.9 proposed.
use axum::routing::{get, post};

fn main() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()
        .unwrap();
    runtime.block_on(async {
        let app = axum::Router::new()
            .route("/health", get(|| async { http_stack::BODY }))
            .route(
                "/v1/chat/completions",
                post(|_body: String| async {
                    ([("content-type", "application/json")], http_stack::BODY)
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        println!("{}", listener.local_addr().unwrap());
        axum::serve(listener, app).await.unwrap();
    });
}
