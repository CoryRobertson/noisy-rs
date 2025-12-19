use crate::event::Event;
use crate::webserver::Procedure;
use chrono::Utc;
use cr_lommy::AllArgsConstructor;
use serenity::all::{
    ActivityData, Context, CreateMessage, EventHandler, GuildId, Message, MessageBuilder, Ready,
};
use serenity::{async_trait, FutureExt};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;
use tokio::sync::Mutex;
use tracing::log::{debug, error, info};

/// this is the bot handler
#[derive(AllArgsConstructor, Debug)]
pub struct Handler {
    /// this is a boolean that checks if we have already spawned the threads that the bot use
    thread_running: AtomicBool,
    bot_state: BotState,
}

impl Handler {
    pub fn new(rx: Receiver<Procedure>) -> Self {
        // TODO: if we need to persist any data, we should do it here, where we check for a file that we care about, then deserialize it, and if it ends up being not present or no good, then we use a default value

        Self {
            thread_running: AtomicBool::new(false),
            bot_state: BotState::new(rx),
        }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn cache_ready(&self, ctx: Context, guilds: Vec<GuildId>) {
        info!("Cache built successfully!");

        // Store a copy of all the guilds that the bot is connected to, so we can reference them in the future
        *self.bot_state.guilds.lock().await = guilds;

        // this context clone is so the async threads can have access to their own bot contexts
        let ctx = Arc::new(ctx);

        if !self.thread_running.load(Ordering::Relaxed) {
            setup_bot_threads(self.bot_state.clone(), ctx.clone()).await;

            self.thread_running.store(true, Ordering::Relaxed);
        }
    }

    async fn message(&self, ctx: Context, msg: Message) {
        if msg.content == "!ping" {
            if let Err(why) = msg.channel_id.say(&ctx.http, "Pong!").await {
                error!("Error sending message: {why:?}");
            }
        }

        if msg.content == "!setup" {
            let builder = MessageBuilder::new()
                .push("React to test this stuff!")
                .build();
            match msg.channel_id.say(&ctx.http, builder).await {
                Ok(message_sent) => {
                    if let Err(err) = message_sent.react(ctx.http, '😀').await {
                        error!("Error reacting to message: {err:?}");
                    }
                }
                Err(why) => error!("Error sending message: {why:?}"),
            }
        }
    }

    async fn ready(&self, _ctx: Context, data_about_bot: Ready) {
        info!("{} is connected!", data_about_bot.user.name);
    }
}

#[derive(Debug)]
pub struct BotState {
    /// this is a web event receiver that will prompt the bot to message all users
    receiver: Receiver<Procedure>,
    /// A vector of all the guilds that the bot is connected to
    guilds: Arc<Mutex<Vec<GuildId>>>,
    events: Arc<Mutex<Vec<Event>>>,
}

impl Clone for BotState {
    fn clone(&self) -> Self {
        Self {
            receiver: self.receiver.resubscribe(),
            guilds: self.guilds.clone(),
            events: self.events.clone(),
        }
    }
}

impl BotState {
    pub fn new(receiver: Receiver<Procedure>) -> Self {
        Self {
            receiver,
            guilds: Arc::default(),
            events: Arc::default(),
        }
    }
}

async fn send_user_message(ctx: &Context, name: Event, message_id: Arc<Mutex<Option<Message>>>) {
    let lock = message_id.lock().await;

    match lock.clone() {
        None => {}
        Some(message) => {
            match message.reaction_users(&ctx.http, '😀', None, None).await {
                Ok(reaction_users) => {
                    // TODO: something we can also do is get the channel that the message is in (or just store a channel and not have a reaction message at all), then iterate through the users
                    //  and compare each of their usernames with usernames sent through the WebEvent that gets received
                    //  -> https://docs.rs/serenity/latest/serenity/model/channel/struct.Message.html#method.channel
                    //  -> https://docs.rs/serenity/latest/serenity/model/channel/struct.GuildChannel.html#method.members

                    for user in reaction_users.iter().filter(|user| !user.bot) {
                        match user
                            .direct_message(&ctx.http, CreateMessage::new().content("test"))
                            .await
                        {
                            Ok(_) => {}
                            Err(why) => error!("Error sending message: {why:?}, {user:?}"),
                        }
                    }
                }
                Err(err) => {
                    error!("Error getting reaction users: {err:?}");
                }
            }
        }
    }
    todo!() // probably delete this code once we are sure there is nothing we want to pull from it
}

#[tracing::instrument]
async fn setup_bot_threads(bot_state: BotState, ctx: Arc<Context>) {
    tokio::spawn(set_activity_to_current_time(ctx.clone()));
    tokio::spawn(react_to_procedures(bot_state.clone(), ctx.clone()));
    tokio::spawn(send_notifications(bot_state.clone(), ctx.clone()));
    info!("Finished spawning bot threads");
}

#[tracing::instrument(skip(ctx, bot_state))]
async fn send_notifications(bot_state: BotState, ctx: Arc<Context>) {
    loop {
        // TODO
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

#[tracing::instrument(skip(ctx, bot_state))]
async fn react_to_procedures(mut bot_state: BotState, ctx: Arc<Context>) {
    loop {
        match bot_state.receiver.recv().await {
            Ok(procedure) => {
                info!("New procedure received: {procedure:?}");

                match procedure {
                    Procedure::NewEvent(_new_event) => {
                        // example implementation
                        for member_list_future in ctx.cache.guilds().iter().map(|g| g.members(&ctx.http, None, None)) {
                            for name in member_list_future.await.iter().flatten().map(|m| m.display_name().clone()) {
                                info!("name: {name}");
                            }
                        }

                        // maybe add the event we received here into the bot state
                    }
                }
            }
            Err(err) => {
                error!("Error receiving procedure: {err:?}");
            }
        }

        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

#[tracing::instrument(skip(ctx))]
async fn set_activity_to_current_time(ctx: Arc<Context>) {
    loop {
        let current_time = Utc::now();
        let formatted_time = current_time.to_rfc2822();

        ctx.set_activity(Some(ActivityData::playing(formatted_time)));

        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}
