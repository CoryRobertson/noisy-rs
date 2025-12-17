use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, NaiveDateTime, Utc};
use serenity::all::{GatewayIntents, GuildId, Message, MessageBuilder, Ready, UserId};
use serenity::builder::CreateMessage;
use serenity::gateway::ActivityData;
use serenity::prelude::{Context, EventHandler};
use serenity::{async_trait, Client};
use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use axum::body::Body;
use axum::http::Response;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast::{Receiver, Sender};
use tokio::sync::Mutex;

#[derive(Clone, Deserialize, Serialize)]
struct Guest {
    user_id: String,
    notify_amount: u32
}

#[derive(Clone, Deserialize, Serialize)]
struct NewEvent {
    event_id: String,
    start_time: NaiveDateTime,
    end_time: NaiveDateTime,
    event_type: String,
    event_title: String,
    guest_list: Vec<Guest>
}


/// this is the bot handler
struct Handler {
    /// this is a boolean that checks if we have already spawned the threads that the bot use
    thread_running: AtomicBool,
    /// this is a web event receiver that will prompt the bot to message all users
    receiver: Receiver<Procedure>,
    /// this is the message that the bot will keep track of reactions from
    /// eventually we probably want to persist this message field so we don't have to run the setup command every time the bot starts
    /// we could also probably store a list of them, or a map of them that has different categories so we can have messages sent out to multiple different lists of people
    react_message: Arc<Mutex<Option<Message>>>,
}


/// A example struct that can be communicated from the web server to the bot, and then hopefully to the users who are subscribed to it
#[derive(Clone)]
struct WebEvent {
    do_thing: bool,
    name: String, // dummy field
}



#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.content == "!ping" {
            if let Err(why) = msg.channel_id.say(&ctx.http, "Pong!").await {
                println!("Error sending message: {why:?}");
            }
        }

        if msg.content == "!setup" {
            let builder = MessageBuilder::new().push("React to test this stuff!").build();
            match msg.channel_id.say(&ctx.http, builder).await {
                Ok(message_sent) => {
                    let mut lock = self.react_message.lock().await;

                    match lock.as_mut() {
                        None => {
                            *lock = Some(message_sent.clone());
                        }
                        Some(message_id) => {
                            *message_id = message_sent.clone();
                        }
                    }

                    if let Err(err) = message_sent.react(ctx.http, '😀').await {
                        println!("Error reacting to message: {err:?}");
                    }

                },
                Err(why) => println!("Error sending message: {why:?}"),
            }
        }
    }

    async fn ready(&self, _ctx: Context, data_about_bot: Ready) {
        println!("{} is connected!", data_about_bot.user.name);
    }



    async fn cache_ready(&self, ctx: Context, _guilds: Vec<GuildId>) {
        println!("Cache built successfully!");

        let ctx = Arc::new(ctx);
        let recv = self.receiver.resubscribe();

        if !self.thread_running.load(Ordering::Relaxed) {
            let ctx1 = Arc::clone(&ctx);
            let mut recv1 = recv.resubscribe();
            let react_message = self.react_message.clone();
            tokio::spawn(async move {
                loop {
                    set_activity_to_current_time(&ctx1);

                    if let Ok(event) = recv1.try_recv() {
                        // send_user_message(&ctx1, event, react_message.clone()).await;
                        //todo: work from here
                    }

                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            });

            self.thread_running.store(true, Ordering::Relaxed);
        }
    }
}

async fn send_user_message(ctx: &Context, name: WebEvent, message_id: Arc<Mutex<Option<Message>>>, ) {
    let lock = message_id.lock().await;

    match lock.clone() {
        None => {}
        Some(message) => {
            match message.reaction_users(&ctx.http,'😀',None,None,).await {
                Ok(reaction_users) => {

                    // TODO: something we can also do is get the channel that the message is in (or just store a channel and not have a reaction message at all), then iterate through the users
                    //  and compare each of their usernames with usernames sent through the WebEvent that gets received
                    //  -> https://docs.rs/serenity/latest/serenity/model/channel/struct.Message.html#method.channel
                    //  -> https://docs.rs/serenity/latest/serenity/model/channel/struct.GuildChannel.html#method.members

                    for user in reaction_users.iter().filter(|user| !user.bot) {
                        match user.direct_message(&ctx.http,CreateMessage::new().content("test")).await {
                            Ok(_) => {}
                            Err(why) => println!("Error sending message: {why:?}, {user:?}"),
                        }
                    }

                }
                Err(err) => {
                    println!("Error getting reaction users: {err:?}");
                }
            }
        }
    }

}

fn set_activity_to_current_time(ctx: &Context) {
    let current_time = Utc::now();
    let formatted_time = current_time.to_rfc2822();

    ctx.set_activity(Some(ActivityData::playing(formatted_time)));
}


#[tokio::main]
async fn main() {
    println!("Hello, world!");
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment variable DISCORD_TOKEN");

    // no idea if this is the right amount of perms
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILDS
        | GatewayIntents::GUILD_MESSAGE_REACTIONS
        | GatewayIntents::DIRECT_MESSAGE_REACTIONS;


    let (tx, rx) = tokio::sync::broadcast::channel(32);

    let mut client = Client::builder(&token, intents).event_handler(Handler {
        thread_running: AtomicBool::new(false),
        receiver: rx,
        react_message: Arc::new(Mutex::default())
    }).await.expect("Err creating client");

    let webserver = tokio::spawn(start_webserver(tx));



    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
        webserver.abort();
    }



    webserver.await.unwrap();
}


/// handles the webserver creation, basically just an input to the bot
async fn start_webserver(sender: Sender<Procedure>) {
    let app = Router::new()
        .route("/new_event", post(handle_new_event))
        .with_state(Arc::new(WebserverState {
            sender: sender.clone(),
        }));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener,app).await.unwrap();
}

async fn handle_new_event(
    State(state): State<Arc<WebserverState>>,
    Json(event): Json<NewEvent>,
) -> Response<Body> {
    let _ = state.sender.send(Procedure::NEW_EVENT(event));

    Response::builder().status(200).body(Body::empty()).unwrap()
}

/// shared state for the webserver, this is the event sender that the bot will listen to
struct WebserverState {
    sender: Sender<Procedure>,
}

#[derive(Clone)]
enum Procedure {
    NEW_EVENT(NewEvent),
}