use std::fs::File;
use std::io::{BufRead, BufReader};
use crate::event::Event;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, Response};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use serenity::all::Timestamp;
use tokio::sync::broadcast::Sender;
use tracing::warn;

/// handles the webserver creation, basically just an input to the bot
#[tracing::instrument]
pub async fn start_webserver(sender: Sender<Procedure>) {
    let app = Router::new()
        .route("/new_event", post(handle_new_event))
        .route("/get_logs/{page}", get(return_logs))
        .route("/test_page", get(test_page))
        .with_state(Arc::new(WebserverState {
            sender: sender.clone(),
        }));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[derive(Serialize, Deserialize)]
struct Embellishment {
    logs: Vec<LogLine>
}

#[derive(Serialize, Deserialize)]
struct LogLine {
    timestamp: String,
    level: String,
    fields: Field,
    target: String,
    span: Span,
}

#[derive(Serialize, Deserialize)]
struct Field {
    message: String,
}

#[derive(Serialize, Deserialize)]
struct Span {
    state: String,
    name: String,
}

async fn return_logs(Path(page): Path<usize>) -> Json<Embellishment>{
    let file = File::open("logs/logs.log").unwrap();
    let br = BufReader::new(file);
    let mut lines = br.lines()
        .skip(page*500)
        .take(500)
        .filter_map(|s| s.ok())
        .filter_map(|s| serde_json::from_str(&s).ok())
        .collect::<Vec<LogLine>>();



    let e: Embellishment = Embellishment{logs: lines};
    // let content = serde_json::to_string(&e).unwrap();

    Json(e)
}

#[tracing::instrument]
async fn test_page(State(state): State<Arc<WebserverState>>) -> impl IntoResponse {
    #[cfg(debug_assertions)]
    state
        .sender
        .send(Procedure::NewEvent(Event::default()))
        .unwrap();

    warn!("test");
    "Yay"
}

#[tracing::instrument]
async fn handle_new_event(
    State(state): State<Arc<WebserverState>>,
    Json(event): Json<Event>,
) -> Response<Body> {
    let _ = state.sender.send(Procedure::NewEvent(event));

    Response::builder().status(200).body(Body::empty()).unwrap()
}

/// shared state for the webserver, this is the event sender that the bot will listen to
#[derive(Debug)]
pub struct WebserverState {
    sender: Sender<Procedure>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
/// Enum representing an action that the webserver can send to the bot for the bot to handle
pub enum Procedure {
    /// Creates a new event that is tracked by the bot
    NewEvent(Event),
}
