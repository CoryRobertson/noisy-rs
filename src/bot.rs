use std::fs::File;
use std::io::{Read, Write};
use crate::event::{Event, EventResponse};
use crate::webserver::Procedure;
use chrono::{Local, Utc};
use serenity::all::{ActivityData, Channel, Context, CreateMessage, EventHandler, GuildId, Member, Message, MessageBuilder, Ready, UserId};
use serenity::async_trait;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use serenity::builder::GetMessages;
use serenity::futures::StreamExt;
use tokio::sync::broadcast::Receiver;
use tokio::sync::Mutex;
use tracing::log::{error, info, warn};

mod notifications;
mod bot_state;

pub use bot_state::BotState;


/// this is the bot handler
#[derive(Debug)]
pub struct Handler {
    /// this is a boolean that checks if we have already spawned the threads that the bot use
    thread_running: AtomicBool,
    /// The state of the bot that may be needed across threads
    /// This struct is clonable with minimal cost
    bot_state: BotState,
}

impl Handler {
    pub fn new(rx: Receiver<Procedure>) -> Self {
        Self {
            thread_running: AtomicBool::new(false),
            bot_state: BotState::new(rx),
        }
    }

    pub fn bot_state(&self) -> &BotState {
        &self.bot_state
    }
}

#[tracing::instrument(skip(ctx, bot_state))]
/// Spawns threads that the bot will use to do the following:
/// Set the current time in the bots about me on discord
/// React to procedures sent by the webserver
/// Spawn notification threads when needed
async fn setup_bot_threads(bot_state: BotState, ctx: Arc<Context>) {
    tokio::spawn(set_activity_to_current_time(ctx.clone()));
    tokio::spawn(react_to_procedures(bot_state.clone(), ctx.clone()));
    tokio::spawn(notifications::spawn_notification_threads(bot_state.clone(), ctx.clone()));
    // TODO: make a async future that loops through the bot state events, checks which events have happened more than one day ago, and then delete them from the list.
    info!("Finished spawning bot threads");
}

#[async_trait]
impl EventHandler for Handler {
    #[tracing::instrument(skip(self, ctx, guilds))]
    async fn cache_ready(&self, ctx: Context, guilds: Vec<GuildId>) {
        info!("Cache built successfully!");

        // Store a copy of all the guilds that the bot is connected to, so we can reference them in the future
        self.bot_state.bot_state_data().lock().await.guilds = guilds;
        let _ = self.bot_state.bot_context().lock().await.insert(Arc::new(ctx.clone()));

        // this context clone is so the async threads can have access to their own bot contexts
        let ctx = Arc::new(ctx);

        if !self.thread_running.load(Ordering::Relaxed) {
            setup_bot_threads(self.bot_state.clone(), ctx.clone()).await;

            self.thread_running.store(true, Ordering::Relaxed);
        }
    }

    #[tracing::instrument(skip(self, ctx))]
    async fn message(&self, ctx: Context, msg: Message) {
        match msg.content.to_lowercase().as_str() {
            "!clear" => {
                if let Ok(channel) = msg.channel(&ctx.http).await {
                    match channel {
                        Channel::Guild(_) => {
                            // TODO: maybe allow the bot to clear messages it puts in text channels too ??
                        }
                        Channel::Private(dm_channel) => {
                            if let Ok(messages) = dm_channel.messages(&ctx.http,GetMessages::new()).await {
                                info!("Clear command received, clearing messages in direct message list");
                                for message in messages {
                                    if message.author.bot {
                                        // lazy try to delete message, if we fail who cares
                                        let _ = message.delete(&ctx.http).await;
                                    }
                                }
                            }
                        }
                        _ => {
                            warn!("Channel type unknown for {msg:?}, {channel:?}");
                        }
                    }
                }
            }
            "!setup" => {
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
            "!verifyusername" => {
                // TODO: pre wrote but none of this seems to be needed? diagram?
                todo!()
                // let bsd = self.bot_state.bot_state_data();
                // let mut lock = bsd.lock().await;
                // 
                // if let Some((idx,(found_user, random_number))) = lock.waiting_to_verify.iter().cloned().enumerate().find(|(_,(waiting, _))| waiting.id == msg.author.id) {
                //     lock.waiting_to_verify.remove(idx);
                //     info!("Found user waiting to verify in waiting list, removing from list");
                // 
                // 
                // }
            }
            "!ping" => {
                if let Err(why) = msg.channel_id.say(&ctx.http, "Pong!").await {
                    error!("Error sending message: {why:?}");
                }
            }
            _ => {}
        }
    }

    #[tracing::instrument(skip(self, _ctx, data_about_bot))]
    async fn ready(&self, _ctx: Context, data_about_bot: Ready) {
        info!("{} is connected!", data_about_bot.user.name);
    }
}

#[allow(dead_code)]
/// Unused function that was used to demonstrate what things we might need
async fn send_user_message(ctx: &Context, _name: Event, message_id: Arc<Mutex<Option<Message>>>) {
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


/// A common function that will delay the bot by a set amount, this is a common function because we want all rate limiting to be done with the same function so it can share data if it needs to
pub(super) async fn rate_limit_delay() {
    tokio::time::sleep(Duration::from_secs(1)).await;
}

#[tracing::instrument(skip(_ctx, bot_state))]
/// Bot thread that awaits procedures from the webserver and handles them accordingly
async fn react_to_procedures(mut bot_state: BotState, _ctx: Arc<Context>) {
    loop {
        match bot_state.receiver_mut().recv().await {
            Ok(procedure) => {
                info!("New procedure received: {procedure:?}");

                match procedure {
                    Procedure::NewEvent(new_event) => {
                        bot_state.bot_state_data().lock().await.events.push(new_event);
                    }
                    Procedure::SetRSVP(rsvp) => {
                        info!("RSVP bot procedure received: {rsvp:?}");
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

pub async fn find_discord_user_with_name(username: &str, ctx: Arc<Context>, bot_state: BotState) -> Vec<Member> {
    let bsd = bot_state.bot_state_data().clone();
    let lock = bsd.lock().await;
    let searched = lock.guilds.iter()
        .map(|g| {g.search_members(&ctx.http,username,None)});

    let mut users_found = vec![];

    for search in searched {
        let search_result = search.await;
        match search_result {
            Ok(found) => {
                users_found.extend_from_slice(&found);
            }
            Err(err) => {
                error!("Error searching for members: {err:?}, with username: {}", username);
            }
        }
    }

    info!("Found {} users with username or nickname of: {}", users_found.len(), username);
    users_found
}

#[tracing::instrument(skip(ctx))]
/// Bot thread that sets the current time to the bots discord about me section on a fixed delay
async fn set_activity_to_current_time(ctx: Arc<Context>) {
    loop {
        let current_time = Utc::now();
        let formatted_time = current_time.to_rfc2822();

        ctx.set_activity(Some(ActivityData::playing(formatted_time)));

        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}
