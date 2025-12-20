use std::env;
use std::sync::Arc;
use eframe::Frame;
use egui::Context;
use serenity::all::{GatewayIntents, Member, User};
use serenity::builder::CreateMessage;
use serenity::Client;
use tokio::runtime::{EnterGuard, Runtime};
use tokio::sync::broadcast::Sender;
use tokio::sync::Mutex;
use noisy_rs::bot::{BotState, Handler};
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
    bot_state: BotState,
    users: Vec<Member>,
    input_field: String,
    selected_user: Option<User>,
}

impl eframe::App for BotApp {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if ui.button("Refresh users").clicked() {
                match self.extract_ctx() {
                    None => {}
                    Some(ctx) => {
                        self.users = ctx.cache.guilds().iter().map(|guild| self.runtime.block_on(guild.members(&ctx.http,None,None)))
                            .flatten()
                            .flatten()
                            .collect();
                    }
                }
            }

            ui.label("Input box:");
            ui.text_edit_singleline(&mut self.input_field);


            egui::ScrollArea::vertical()
                .show(ui,|ui| {
                for member_chunk in self.users.as_slice().chunks(3) {
                    ui.horizontal(|ui| {
                        for m in member_chunk {
                            let disable_button = self.selected_user.as_ref().is_some_and(|u| u == &m.user);

                            ui.add_enabled_ui(!disable_button, |ui| {
                                if ui.button(format!("{}", m)).clicked() {
                                    self.selected_user = Some(m.user.clone());
                                }
                            });
                        }
                    });
                }
            });

            ui.separator();

            if let Some(user) = self.selected_user.as_ref() {
                if ui.button(format!("Send to {}", user.display_name())).clicked() {
                    if let Some(ctx) = self.extract_ctx() {
                        let _ = self.runtime.block_on(user.direct_message(&ctx.http,CreateMessage::new().content(self.input_field.clone())));
                    }
                }
            }


        });
    }
}

impl BotApp {
    fn extract_ctx(&self) -> Option<serenity::client::Context> {
        let ctx = self.bot_state.get_context().clone();
        self.runtime.block_on(ctx.lock()).clone()
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
        let bot_state = bot_handler.get_state().clone();

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
            bot_state,
            users: Vec::new(),
            input_field: String::new(),
            selected_user: None,
        }
    }
}