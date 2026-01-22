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
use serenity::all::CreateMessage;
use tokio::sync::broadcast::Sender;
use tracing::{info, warn};
use crate::bot::{find_discord_user_with_name, BotState};

/// handles the webserver creation, basically just an input to the bot
#[tracing::instrument]
pub async fn start_webserver(sender: Sender<Procedure>, bot_state: BotState) {
    info!("Starting webserver");
    let app = Router::new()
        .route("/new_event", post(handle_new_event))
        .route("/set_guest_response", post(set_guest_response))
        .route("/get_logs/{page}", get(return_logs))
        .route("/test_page", get(test_page))
        .route("/verify_username/{username}/{random_number}", get(start_verify_username))
        .with_state(Arc::new(WebserverState {
            sender: sender.clone(),
            bot_state,
        }));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[tracing::instrument]
async fn start_verify_username(
    State(state): State<Arc<WebserverState>>,
    Path(discord_username): Path<String>,
    Path(random_number): Path<u32>,
) -> Response<Body> {
    info!("Starting verification of discord username: {}", discord_username);

    let ctx = state.bot_state.bot_context()
        .lock().await.clone()
        .expect("Bot context not present, this should never be able to happen!");

    let found_users = find_discord_user_with_name(&discord_username,ctx.clone(),state.bot_state.clone()).await;
    if found_users.len() > 2 {
        warn!("Found more than 2 users with discord username: {}", discord_username);
    }

    let bsd = state.bot_state.bot_state_data().clone();
    let mut lock = bsd.lock().await;

    for mem in found_users {
        let _ = mem.user.direct_message(&ctx.http,CreateMessage::new().content(format!("Your verification code is: {}", random_number))).await;

        // TODO: probably dont need this? unsure
        // add user to verification list if they are not already present
        // if lock.waiting_to_verify().iter().find(|(u,_)| u.id.clone() == mem.user.id.clone()).is_none() {
        //     lock.waiting_to_verify.push((mem.user, random_number));
        // }
        // TODO: we can make a post request or get request with the found usernames and the random number we sent them to event star
    }



    Response::builder().status(200).body(Body::empty()).unwrap()
}


/// Sets the guest's RSVP response and the notification amount
#[tracing::instrument]
async fn set_guest_response(
    State(state): State<Arc<WebserverState>>,
    Json(change_rsvp): Json<ChangeRSVP>,
) -> Response<Body> {
    info!("Setting guest response: {:?}", change_rsvp);
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
                    info!("Found event and guest: {:?}", guest);
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

    info!("Got {} log lines from page: {}", lines.len(), page);
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
    info!("Adding new event: {:?}", event);
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
