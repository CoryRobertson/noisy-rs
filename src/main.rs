use noisy_rs::bot::Handler;
use noisy_rs::event::EventResponse;
use noisy_rs::webserver;
use serenity::all::GatewayIntents;
use serenity::Client;
use std::{env, fs};
use tracing::{error, info, warn};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt};

#[tokio::main]
#[tracing::instrument]
async fn main() {
    let file_appender = tracing_appender::rolling::never("./logs", "logs.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    let file = fmt::layer()
        .json()
        .with_writer(non_blocking)
        .with_ansi(false);
    let subscriber = tracing_subscriber::fmt().finish();
    subscriber.with(file).init();

    #[cfg(debug_assertions)]
    if option_env!("DEBUG_JSON").is_some() {
        output_debug_json();
        return; // end program after outputting debug json
    }

    let token = env::var("DISCORD_TOKEN")
        .expect("Expected a token in the environment variable DISCORD_TOKEN");

    // no idea if this is the right amount of perms
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILDS
        | GatewayIntents::GUILD_MESSAGE_REACTIONS
        | GatewayIntents::DIRECT_MESSAGE_REACTIONS;

    let (procedure_sender, procedure_receiver) = tokio::sync::broadcast::channel(32);

    let bot_event_handler = Handler::new(procedure_receiver);
    let bot_state = bot_event_handler.bot_state().clone();

    let mut client = Client::builder(&token, intents)
        .event_handler(bot_event_handler)
        .await
        .expect("Err creating client");
    
    tokio::select! {
        res_client = client.start() => {
            if let Err(why) = res_client {
                error!("Client error: {:?}", why);
            }
        }
        _ = webserver::start_webserver(procedure_sender, bot_state) => {
            error!("Webserver future finished, this is not intended to happen");
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Ctrl-C received, shutting down...");
        }
    }
}

#[cfg(debug_assertions)]
#[tracing::instrument]
fn output_debug_json() {
    let event = noisy_rs::event::Event::default();
    let guest = noisy_rs::event::Guest::new("cool_test_user", 1);
    let change_rsvp = noisy_rs::event::ChangeRSVP {
        responded: EventResponse::Going,
        event_id: "123".to_string(),
        user_id: "234".to_string(),
    };

    let json_event = serde_json::to_string(&event).unwrap();
    let json_guest = serde_json::to_string(&guest).unwrap();
    let json_change_rsvp = serde_json::to_string(&change_rsvp).unwrap();

    fs::write("event.json", json_event).unwrap();
    fs::write("guest.json", json_guest).unwrap();
    fs::write("changeRsvp.json", json_change_rsvp).unwrap();
}
