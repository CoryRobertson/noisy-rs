use std::env;
use std::sync::Arc;
use eframe::Frame;
use egui::Context;
use serenity::all::GatewayIntents;
use serenity::Client;
use tokio::runtime::{EnterGuard, Runtime};
use tokio::sync::broadcast::Sender;
use tokio::sync::Mutex;
use noisy_rs::bot::Handler;
use noisy_rs::webserver::{start_webserver, Procedure};

fn main() {
    tracing_subscriber::fmt::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
        ..Default::default()
    };

    eframe::run_native("Gui for bot??",options,Box::new(|cc| {
        Ok(Box::<BotApp>::default())
    }))
        .unwrap();
}

struct BotApp {
    runtime: Arc<Runtime>,
    sender: Sender<Procedure>,
    web_server: tokio::task::JoinHandle<()>,
    client_handle: tokio::task::JoinHandle<()>,
    ctx: Arc<Mutex<Option<serenity::client::Context>>>,
}

impl eframe::App for BotApp {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("AAA");
        });
    }
}

impl Default for BotApp {
    fn default() -> Self {
        let rt = Runtime::new().unwrap();

        let (procedure_sender, procedure_receiver) = tokio::sync::broadcast::channel(32);

        let intents = GatewayIntents::GUILD_MESSAGES
            | GatewayIntents::DIRECT_MESSAGES
            | GatewayIntents::MESSAGE_CONTENT
            | GatewayIntents::GUILDS
            | GatewayIntents::GUILD_MESSAGE_REACTIONS
            | GatewayIntents::DIRECT_MESSAGE_REACTIONS;

        let token = env::var("DISCORD_TOKEN")
            .expect("Expected a token in the environment variable DISCORD_TOKEN");

        let web_server = rt.spawn(start_webserver(procedure_sender.clone()));

        let bot_handler = Handler::new(procedure_receiver.resubscribe());
        let ctx = bot_handler.get_state().get_context().clone();

        let client_handle = rt.spawn(async move {
            let mut client = Client::builder(&token, intents)
                .event_handler(bot_handler)
                .await
                .expect("Err creating client");


            if let Err(why) = client.start().await {
                println!("Client error: {:?}", why);
            }
        });

        Self {
            runtime: Arc::new(rt),
            sender: procedure_sender,
            web_server,
            client_handle,
            ctx,
        }
    }
}