use std::fs::File;
use std::io::{Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use cr_lommy::Getters;
use tracing::log::{error, info};
use tokio::sync::broadcast::Receiver;
use tokio::sync::Mutex;
use serde::{Deserialize, Serialize};
use serenity::all::{GuildId, User};
use serenity::prelude::Context;
use crate::event::Event;
use crate::webserver::Procedure;

#[derive(Debug, Getters)]
pub struct BotState {
    /// this is a web event receiver that will prompt the bot to message all users
    receiver: Receiver<Procedure>,
    #[getters_lommy_skip]
    bot_state_data: Arc<Mutex<BotStateData>>,
    bot_context: Arc<Mutex<Option<Context>>>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, Getters)]
pub struct BotStateData {
    /// A vector of all the guilds that the bot is connected to
    pub(super) guilds: Vec<GuildId>,
    pub(super) events: Vec<Event>,
    #[serde(skip)]
    pub waiting_to_verify: Vec<(User, u32)>,
}

impl BotStateData {

    pub fn guilds_mut(&mut self) -> &mut Vec<GuildId> {
        &mut self.guilds
    }



    pub fn events_mut (&mut self) -> &mut Vec<Event> {
        &mut self.events
    }

    pub fn save(&self) {
        let clone_to_drop = self.clone();
        drop(clone_to_drop);
    }
}

/// Handles saving the bot state data when it is dropped, this guarantees that the bot state is dropped with a SIG CTRL_C, but not with SIG TERM of other process ending methods
impl Drop for BotStateData {
    fn drop(&mut self) {
        info!("Saving BotStateData");

        match File::options().read(true).write(true).create(true).open("persistent_save.json") {
            Ok(mut file) => {
                // we can unwrap here because we are already closing the program!
                let data_string = serde_json::to_string(self).unwrap();
                file.write_all(data_string.as_bytes()).unwrap();
                info!("Successfully saved BotStateData");
            }
            Err(err) => {
                error!("Unable to open persistent_save.json file: {err:?}");
            }
        }


    }
}

impl Clone for BotState {
    fn clone(&self) -> Self {
        Self {
            receiver: self.receiver.resubscribe(),
            bot_state_data: self.bot_state_data.clone(),
            bot_context: Arc::new(Mutex::new(None)),
        }
    }
}

impl BotState {
    /// Creates a new `BotState`, this function contains logic that will not allow it to be called more than once
    pub(super) fn new(receiver: Receiver<Procedure>) -> Self {

        static CALL_COUNT: AtomicUsize = AtomicUsize::new(0);

        let call_count = CALL_COUNT.fetch_add(1, Ordering::Relaxed);

        debug_assert!(call_count == 0);
        if call_count > 0 {
            error!("Too many BotState structs have been instantiated, make sure they are only being cloned, call_count: {}", call_count);
        }

        match File::options().read(true).write(true).create(true).open("persistent_save.json").map(|mut file| {
            let mut file_content = String::new();
            file.read_to_string(&mut file_content).unwrap();
            file_content
        }).map(|file_content| {
            match serde_json::from_str(&file_content) {
                Ok(data) => {Some(data)},
                Err(err) => {
                    error!("Error deserializing persistent save file: {err:?}");
                    None
                }
            }
        }) {
            Ok(Some(data)) => {
                info!("Successfully loaded persistent save");
                Self {
                    receiver,
                    bot_state_data: Arc::new(Mutex::new(data)),
                    bot_context: Arc::new(Mutex::new(None)),
                }
            }
            _ => {
                info!("Failed to load persistent save");
                Self {
                    receiver,
                    bot_state_data: Arc::new(Mutex::new(BotStateData::default())),
                    bot_context: Arc::new(Mutex::new(None)),
                }
            }
        }
    }

    pub fn bot_state_data(&self) -> Arc<Mutex<BotStateData>> {
        self.bot_state_data.clone()
    }

    pub fn receiver_mut(&mut self) -> &mut Receiver<Procedure> {
        &mut self.receiver
    }


}