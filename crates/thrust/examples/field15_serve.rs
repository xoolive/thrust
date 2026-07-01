use axum::extract::{Json, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::Router;
use serde::{Deserialize, Serialize};
use std::env;
use std::path::Path;
use std::sync::Arc;
use thrust::data::eurocontrol::database::AirwayDatabase;
use thrust::data::field15::Field15Parser;
use tower_http::cors::{Any, CorsLayer};

#[derive(Debug, Deserialize)]
struct RouteRequest {
    route: String,
}

#[derive(Debug, Serialize)]
struct RouteResponse {
    route: String,
    segments: Vec<thrust::data::eurocontrol::database::ResolvedRouteSegment>,
}

struct AppState {
    database: AirwayDatabase,
}

async fn resolve_route(
    State(_state): State<Arc<AppState>>,
    Json(payload): Json<RouteRequest>,
) -> impl IntoResponse {
    eprintln!("Received route to resolve: {}", payload.route);
    let elements = Field15Parser::parse(&payload.route);
    let enriched = _state.database.enrich_route(elements);

    (
        StatusCode::OK,
        Json(RouteResponse {
            route: payload.route.clone(),
            segments: enriched,
        }),
    )
        .into_response()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <path_to_directory>", args[0]);
        std::process::exit(1);
    }
    let path = Path::new(&args[1]);

    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Load the database once at startup
    println!("Loading database...");
    let database = AirwayDatabase::new(path)?;
    println!("Database loaded successfully!");

    // Create shared state
    let state = Arc::new(AppState { database });

    // Configure CORS
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Build the route
    let app = Router::new()
        .route("/resolve", post(resolve_route))
        .with_state(state)
        .layer(cors);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    println!("Server listening on http://127.0.0.1:3000");
    println!("POST to /resolve with JSON: {{\"route\": \"YOUR_ROUTE_STRING\"}}");

    axum::serve(listener, app).await?;

    Ok(())
}
