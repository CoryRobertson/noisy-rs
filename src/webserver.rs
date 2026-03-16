use crate::event::{ChangeRSVP, Event, EventResponse, Guest};
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
use cr_lommy::AllArgsConstructor;
use serenity::all::{CreateMessage, User, UserId};
use serenity::all::standard::Reason::Log;
use tokio::runtime::{Handle};
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
        .route("/get_guest_response", post(get_guest_response))
        .route("/get_logs/{page}", get(return_logs))
        .route("/test_page", get(test_page))
        .route("/discord/verify_user_id/{user_id}/{random_number}", get(start_verify_user_id))
        .route("/discord/search_user/{username}", get(search_user))
        .route("/discord/get_users", post(get_users)) //todo: get user's notification amount for an event
        .with_state(Arc::new(WebserverState {
            sender: sender.clone(),
            bot_state,
        }));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3003").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[derive(Serialize, Deserialize, AllArgsConstructor)]
pub struct DiscordUsernameSearchResponse {
    results: Vec<DiscordUsernameSearchResult>
}

#[derive(Serialize, Deserialize, AllArgsConstructor)]
pub struct DiscordUsernameSearchResult {
    name: String,
    id: String,
    avatar: Option<String>,
    global_name: Option<String>,
}

impl From<User> for DiscordUsernameSearchResult {
    fn from(user: User) -> Self {
        let s = user.avatar_url().clone();

        Self{
            name: user.name,
            id: user.id.get().to_string(),
            avatar: s,
            global_name: user.global_name,
        }
    }
}

/// Received from EventStar. Contains a list of Discord Ids
#[derive(Serialize, Deserialize)]
struct UserIdList {
    list: Vec<String>
}

async fn get_users(
    State(state): State<Arc<WebserverState>>,
    Json(userList): Json<UserIdList>,
) -> Json<DiscordUsernameSearchResponse> {
    info!("Retrieving Discord accounts for matching Discord ids.");

    let ctx = state.bot_state.bot_context()
        .lock().await.clone()
        .expect("Bot context not present, this should never be able to happen!");

    let mut users: Vec<DiscordUsernameSearchResult> = vec![];

    for future in userList.list.iter().filter_map(|uid| {
        uid.parse()
            .map(|r: u64| UserId::new(r).to_user(&ctx.http))
            .ok()
    }) {
        if let Ok(user) = future.await {
            users.push(user.into());
        }
    }

    let resp = DiscordUsernameSearchResponse::new_all_args(users);

    Json(resp)
}

async fn search_user(
    State(state): State<Arc<WebserverState>>,
    Path(discord_username): Path<String>,
) -> Json<DiscordUsernameSearchResponse> {
    info!("Searching for matching Discord accounts: {}", discord_username);

    let ctx = state.bot_state.bot_context()
        .lock().await.clone()
        .expect("Bot context not present, this should never be able to happen!");

    let found_users = find_discord_user_with_name(&discord_username, Arc::from(ctx.clone()), state.bot_state.clone()).await;
    let resp = DiscordUsernameSearchResponse::new_all_args(found_users.into_iter().map(|a| a.user.into()).collect(),);

    Json(resp)
}

#[tracing::instrument]
async fn start_verify_user_id(
    State(state): State<Arc<WebserverState>>,
    Path((discord_id, random_number)): Path<(u64,String)>,
) -> Response<Body> {
    info!("Starting verification of discord username: {}", discord_id);

    let ctx = state.bot_state.bot_context()
        .lock().await.clone()
        .expect("Bot context not present, this should never be able to happen!");

    match UserId::new(discord_id).to_user(&ctx.http).await {
        Ok(user) => {
            match user.direct_message(&ctx.http, CreateMessage::new().content(format!("Your verification code is: {}", random_number))).await {
                Ok(_) => {
                    Response::builder().status(200).body(Body::empty()).unwrap()
                }
                Err(err) => {
                    Response::builder().status(500).body(Body::new(err.to_string())).unwrap()
                }
            }
        }
        Err(err) => {
            Response::builder().status(500).body(Body::new(err.to_string())).unwrap()
        }
    }
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

                    // Optionally add in the response
                    match rsvp.clone().responded{
                        None => {
                            new_guest.set_responded(EventResponse::NO);
                        }
                        Some(r) => {
                            new_guest.set_responded(r);
                        }
                    }

                    event.mut_guest_list().push(new_guest);
                    Response::builder().status(203).body(Body::empty()).unwrap()
                }
                Some(guest) => {
                    info!("Found event and guest: {:?}", guest);

                    // Optionally change the response
                    if let Some(responded) = rsvp.responded.clone() {
                        guest.set_responded(responded);
                    }

                    guest.set_notify_amount(change_rsvp.notify_amount);
                    Response::builder().status(200).body(Body::empty()).unwrap()
                }
            }
        }
    };
    state.sender.send(Procedure::SetRSVP(rsvp)).unwrap();

    response
}


#[derive(Serialize, Deserialize, AllArgsConstructor, Debug)]
pub struct GetGuestResponseRequest {
    user_id: String,
    event_id: String,
}

#[tracing::instrument]
async fn get_guest_response(
    State(state): State<Arc<WebserverState>>,
    Json(get_rsvp): Json<GetGuestResponseRequest>,
) -> impl IntoResponse {
    info!("Getting guest response: {:?}", get_rsvp);

    let l = state.bot_state.bot_state_data();
    let lock = l.lock().await;

    match lock.events().iter().find(|e| {e.event_id() == get_rsvp.event_id}).map(|e| e.guest_list().iter().find(|g|{g.user_id() == get_rsvp.user_id})).flatten() {
        None => {
            Response::builder().status(404).body(Body::new("Unable to find guest and/or response".to_string())).unwrap()
        }
        Some(guest) => {
            // Found event and the guest
            Json(guest.clone()).into_response()
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct Embellishment {
    logs: Vec<LogLine>,
    pages: usize,
    lines: usize,
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

    // Prob not very efficient...
    let mut all_lines = br
        .lines()
        .filter_map(|s| s.ok().map(|s| serde_json::from_str(&s).ok()).flatten())
        .collect::<Vec<LogLine>>();

    let line_count = all_lines.len();

    all_lines.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    let page_content:Vec<LogLine> = all_lines.into_iter().skip(page * 500).take(500).collect();

    info!("Got {} log lines from page: {}", page_content.len(), page);
    let e: Embellishment = Embellishment { logs: page_content, pages:  line_count.div_ceil(500), lines: line_count };

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
