use std::net::SocketAddr;

use axum::{
    http::{header, HeaderMap},
    response::IntoResponse,
    routing::*,
    Router,
};
use tower_http::services::ServeDir;
use chrono::Local;
use maud::Markup;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let app = Router::new()
        .route("/", get(root))
        .route("/slow", get(slow))
        .nest_service("/assets", get_service(ServeDir::new("assets")));

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

// basic handler that responds with a static string
async fn root() -> impl axum::response::IntoResponse {
    outer_template(maud::html! {
        h1."text-6xl mb-4" { "Hey! I'm a sample app that's slow to respond." }

        h3."text-4xl mb-4" {
            "We have a few routes that respond differently."
        }

        h3."text-4xl mb-16" {
            "The root is the only route that responds without any waiting."
        }

        p."text-xl" {
            a."text-blue-400" href="/slow" { "/slow" }
            " responds after 5 seconds with the current time"
        }
        p."text-xl" {
            a."text-blue-400" href="/fast" { "/fast" }
            " responds after 1 second with the current time"
        }
    })
}

fn outer_template(body: Markup) -> Markup {
    maud::html! {
        script src="https://cdn.tailwindcss.com" {}

        body class="flex flex-col items-center justify-center h-screen" {
            (body)
        }
    }
}

fn now_template(title: &str) -> impl IntoResponse {
    let now = Local::now();

    let mut headers = HeaderMap::new();
    headers.insert(header::CACHE_CONTROL, "max-age=60".parse().unwrap());

    let template = outer_template(maud::html! {
        h1 class="text-6xl" { (title) }
        p class="text-4xl" { (now) }

        a class="text-blue-400 pt-16 text-xl" href="/" { "Go back home" }
    });

    (headers, template)
}

// handler that responds after 5 seconds
async fn slow() -> impl IntoResponse {
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    now_template("Slow")
}