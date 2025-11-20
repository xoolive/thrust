use serde::{Deserialize, Serialize};
use std::env;
use std::{path::Path, sync::Arc};
use thrust::data::eurocontrol::database::AirwayDatabase;
use thrust::data::field15::Field15Parser;
use warp::Filter;

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

fn with_state(
    state: Arc<AppState>,
) -> impl Filter<Extract = (Arc<AppState>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || state.clone())
}

async fn resolve_route(payload: RouteRequest, state: Arc<AppState>) -> Result<impl warp::Reply, warp::Rejection> {
    eprintln!("Received route to resolve: {}", payload.route);
    let elements = Field15Parser::parse(&payload.route);
    let enriched = state.database.enrich_route(elements);

    Ok(warp::reply::with_status(
        warp::reply::json(&RouteResponse {
            route: payload.route.clone(),
            segments: enriched,
        }),
        warp::http::StatusCode::OK,
    ))
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
    let database = AirwayDatabase::new(&path)?;
    println!("Database loaded successfully!");

    // Create shared state
    let state = Arc::new(AppState { database });

    // Configure CORS
    let cors = warp::cors()
        .allow_any_origin()
        .allow_methods(vec!["GET", "POST", "OPTIONS"])
        .allow_headers(vec!["Content-Type", "Authorization"]);

    // Build the route
    let resolve = warp::path("resolve")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_state(state))
        .and_then(resolve_route)
        .with(cors);

    println!("Server listening on http://127.0.0.1:3000");
    println!("POST to /resolve with JSON: {{\"route\": \"YOUR_ROUTE_STRING\"}}");

    warp::serve(resolve).run(([127, 0, 0, 1], 3000)).await;

    Ok(())
}
