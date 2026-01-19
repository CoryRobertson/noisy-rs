use crate::event::{ChangeRSVP, Event, Guest};
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{Response};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::sync::Arc;
use tokio::sync::broadcast::Sender;
use tracing::{info, warn};
use crate::bot::BotState;

/// handles the webserver creation, basically just an input to the bot
#[tracing::instrument]
pub async fn start_webserver(sender: Sender<Procedure>, bot_state: BotState) {
    let app = Router::new()
        .route("/new_event", post(handle_new_event))
        .route("/set_guest_response", post(set_guest_response))
        .route("/get_logs/{page}", get(return_logs))
        .route("/test_page", get(test_page))
        .with_state(Arc::new(WebserverState {
            sender: sender.clone(),
            bot_state,
        }));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

/// Sets the guest's RSVP response and the notification amount
#[tracing::instrument]
async fn set_guest_response(
    State(state): State<Arc<WebserverState>>,
    Json(change_rsvp): Json<ChangeRSVP>,
) -> Response<Body> {
    let rsvp = change_rsvp.clone();
    let bsd = state.bot_state.bot_state_data();
    let mut lock = bsd.lock().await;
    let lock_events = lock.events_mut();
    let response = match lock_events
        .iter_mut()
        .find(|e| e.event_id() == change_rsvp.event_id)
    {
        None => {
            tracing::log::warn!("{}: RSVP event not found", change_rsvp.event_id);
            Response::builder().status(400).body(Body::empty()).unwrap()
        }
        Some(event) => {
            match event
                .mut_guest_list()
                .iter_mut()
                .find(|g| g.user_id() == change_rsvp.user_id)
            {
                None => {
                    tracing::log::warn!("{}: Invited guest not found", change_rsvp.event_id);

                    let mut new_guest = Guest::new(rsvp.clone().user_id, rsvp.notify_amount).unwrap();
                    new_guest.set_responded(rsvp.clone().responded);

                    event.mut_guest_list().push(new_guest);
                    Response::builder().status(203).body(Body::empty()).unwrap()
                }
                Some(guest) => {
                    guest.set_responded(change_rsvp.responded);
                    guest.set_notify_amount(change_rsvp.notify_amount);
                    Response::builder().status(200).body(Body::empty()).unwrap()
                }
            }
        }
    };
    state.sender.send(Procedure::SetRSVP(rsvp)).unwrap();

    response
}

#[derive(Serialize, Deserialize, Debug)]
struct Embellishment {
    logs: Vec<LogLine>,
}

#[derive(Serialize, Deserialize, Debug)]
struct LogLine {
    timestamp: String,
    level: String,
    fields: Field,
    target: String,
    span: Option<Span>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Field {
    message: String,
}

#[derive(Serialize, Deserialize, Debug)]
/// These are input variables that will be printed with each log trace if the field variable name is added here as an optional string
struct Span {
    state: Option<String>,
    name: Option<String>,
    page: Option<String>,
    event: Option<String>,
}

#[tracing::instrument]
async fn return_logs(Path(page): Path<usize>) -> Json<Embellishment> {
    let file = File::open("logs/logs.log").unwrap();
    let br = BufReader::new(file);
    let lines = br
        .lines()
        .skip(page * 500)
        .take(500)
        .filter_map(|s| s.ok().map(|s| serde_json::from_str(&s).ok()).flatten())
        .collect::<Vec<LogLine>>();

    info!("Got {} log lines", lines.len());
    let e: Embellishment = Embellishment { logs: lines };

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
    bot_state: BotState,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
/// Enum representing an action that the webserver can send to the bot for the bot to handle
pub enum Procedure {
    /// Creates a new event that is tracked by the bot
    NewEvent(Event),
    SetRSVP(ChangeRSVP),
}
