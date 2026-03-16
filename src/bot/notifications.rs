use std::sync::Arc;
use std::time::Duration;
use serenity::all::{Context, CreateMessage, UserId};
use tracing::log::{error, info};
use chrono::Local;
use crate::bot;
use crate::bot::bot_state::BotState;
use crate::event::{Event, EventResponse};

#[tracing::instrument(skip(ctx))]
/// A notification thread that notifies all people in an event that the event is created
/// Only notifies people who have their `notify_amount` > 0
pub(crate) async fn creation_notification_spawner(event: Event, ctx: Arc<Context>) {
    info!("Sending notification of new event creation");
    for user in event.guest_list().iter().filter(|s| s.notify_amount() > 0) {
        match user.user_id().parse().map(|id| { UserId::new(id) }).map(|id| { id.to_user(&ctx) }) {
            Ok(id_future) => {
                match id_future.await {
                    Ok(discord_user) => {
                        match discord_user.direct_message(&ctx.http, CreateMessage::new().content(format!("Hi, {}! You are invited to {}. You can RSVP here -> https://eventstar.costionline.com/event/{}", discord_user.display_name(), event.event_title(), event.event_id()))).await {
                            Ok(_) => {}
                            Err(err) => {
                                error!("Error sending message: {err:?}");
                            }
                        }
                    }
                    Err(err) => {
                        error!("Error converting user id into user: {err:?}");
                    }
                }
            }
            Err(err) => {
                error!("Error parsing user ID: {err:?}");
            }
        };
        bot::rate_limit_delay().await;
    }
}

#[tracing::instrument(skip(ctx))]
/// A notification thread that notifies people in an event one day before the event takes place
/// Only notifies people who have their `notify_amount` > 1
pub(crate) async fn day_before_notification_spawner(event: Event, ctx: Arc<Context>) {
    info!("Spawned thread for day before notification");
    #[cfg(debug_assertions)]
    tokio::time::sleep(Duration::from_secs(5)).await;
    loop {
        let time_difference = event
            .start_time()
            .signed_duration_since(Local::now().naive_local());
        if time_difference.num_hours() <= 24 {
            info!("{}: Day before notification sending", event.event_title());
            for user in event.guest_list().iter().filter(|s| s.notify_amount() > 1) {
                match user.user_id().parse().map(|id| { UserId::new(id) }).map(|id| { id.to_user(&ctx) }) {
                    Ok(id_future) => {
                        match id_future.await {
                            Ok(discord_user) => {
                                match discord_user.direct_message(&ctx.http, CreateMessage::new().content(format!("Hi, {}! {} is tomorrow! Check it out here -> https://eventstar.costionline.com/event/{}", discord_user.display_name(), event.event_title(), event.event_id()))).await {
                                    Ok(_) => {}
                                    Err(err) => {
                                        error!("Error sending message: {err:?}");
                                    }
                                }
                            }
                            Err(err) => {
                                error!("Error converting user id into user: {err:?}");
                            }
                        }
                    }
                    Err(err) => {
                        error!("Error parsing user ID: {err:?}");
                    }
                };
                bot::rate_limit_delay().await;
            }
            info!("{}: Day before notification finished", event.event_title());
            break;
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

#[tracing::instrument(skip(ctx))]
/// A notification thread that notifies people in an event one hour before the event is to take place
/// Only notifies people who have their `notify_amount` > 0
pub(crate) async fn hour_before_notification_spawner(event: Event, ctx: Arc<Context>) {
    info!("Spawned thread for hour before notification");
    #[cfg(debug_assertions)]
    tokio::time::sleep(Duration::from_secs(10)).await;
    loop {
        let time_difference = event
            .start_time()
            .signed_duration_since(Local::now().naive_local());
        if time_difference.num_hours() <= 1 {
            info!("{}: Hour before notification sending", event.event_title());
            for user in event.guest_list().iter().filter(|s| s.notify_amount() > 0) {
                match user.user_id().parse().map(|id| { UserId::new(id) }).map(|id| { id.to_user(&ctx) }) {
                    Ok(id_future) => {
                        match id_future.await {
                            Ok(discord_user) => {
                                match discord_user.direct_message(&ctx.http, CreateMessage::new().content(format!("Hi, {}! {} is starting in just one hour! Check it out here -> https://eventstar.costionline.com/event/{}", discord_user.display_name(), event.event_title(), event.event_id()))).await {
                                    Ok(_) => {}
                                    Err(err) => {
                                        error!("Error sending message: {err:?}");
                                    }
                                }
                            }
                            Err(err) => {
                                error!("Error converting user id into user: {err:?}");
                            }
                        }
                    }
                    Err(err) => {
                        error!("Error parsing user ID: {err:?}");
                    }
                };
                bot::rate_limit_delay().await;
            }
            info!("{}: Hour before notification finished", event.event_title());
            break;
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

#[tracing::instrument(skip(ctx))]
/// A notification thread that notifies people that the rsvp due date is due in one hour
/// Only notifies people who have their `notify_amount` > 1
pub(crate) async fn rsvp_hour_before_notification_spawner(event: Event, ctx: Arc<Context>) {
    info!("Spawned thread for rsvp due date notification");
    #[cfg(debug_assertions)]
    tokio::time::sleep(Duration::from_secs(15)).await;
    loop {
        let time_difference = event
            .rsvp_due()
            .signed_duration_since(Local::now().naive_local());
        if time_difference.num_minutes() <= 60 {
            info!(
                "{}: rsvp_hour_before_notification_spawner sending",
                event.event_title()
            );
            for user in event
                .guest_list()
                .iter()
                .filter(|s| s.notify_amount() > 1)
                .filter(|s| s.responded() == EventResponse::NO)
            {
                match user.user_id().parse().map(|id| { UserId::new(id) }).map(|id| { id.to_user(&ctx) }) {
                    Ok(id_future) => {
                        match id_future.await {
                            Ok(discord_user) => {
                                match discord_user.direct_message(&ctx.http, CreateMessage::new().content(format!("Hi, {}! Your RSVP for {} is due in an hour. Check it out here -> https://eventstar.costionline.com/event/{}", discord_user.display_name(), event.event_title(), event.event_id()))).await {
                                    Ok(_) => {}
                                    Err(err) => {
                                        error!("Error sending message: {err:?}");
                                    }
                                }
                            }
                            Err(err) => {
                                error!("Error converting user id into user: {err:?}");
                            }
                        }
                    }
                    Err(err) => {
                        error!("Error parsing user ID: {err:?}");
                    }
                };

                bot::rate_limit_delay().await;
            }
            info!(
                "{}: rsvp_hour_before_notification_spawner finished",
                event.event_title()
            );
            break;
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

#[tracing::instrument(skip(ctx, bot_state))]
/// Spawns all the needed threads to handle sending notifications from the bot
pub(super) async fn spawn_notification_threads(bot_state: BotState, ctx: Arc<Context>) {
    loop {
        {
            let bsd = bot_state.bot_state_data().clone();
            let mut lock = bsd.lock().await;
            for event in lock.events_mut() {
                if !event.notify_threads_spawned() {
                    event.set_notify_threads_spawned(true);

                    tokio::spawn(creation_notification_spawner(event.clone(), ctx.clone()));
                    tokio::spawn(hour_before_notification_spawner(event.clone(), ctx.clone()));
                    tokio::spawn(day_before_notification_spawner(event.clone(), ctx.clone()));
                    tokio::spawn(rsvp_hour_before_notification_spawner(event.clone(), ctx.clone()));
                    // TODO: more notifications ??
                }
            }
            // maybe eventually make this run less often, cause this is wasted write to hard drive time honestly
            lock.save();
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}
