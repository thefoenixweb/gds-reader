mod api;
mod query;
mod state;

use axum::Router;
use state::AppState;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() {
    let state = AppState::new();

    let app = Router::new()
        .merge(api::layout::router())
        .merge(api::layers::router())
        .with_state(state)
        .layer(CorsLayer::permissive())
        .fallback_service(ServeDir::new("../../../frontend/dist"));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Server running on http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}
