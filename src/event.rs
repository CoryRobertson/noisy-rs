use chrono::{Local, NaiveDateTime, TimeDelta};
use serde::{Deserialize, Serialize};
use std::ops::Add;
use serenity::futures::future::ok;
use tracing::error;

#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct Guest {
    user_id: String,
    /// 0: no notifications at all
    /// 1: event created, 1 hour before event start
    /// 2: event created, 1 hour before RSVP due date, 1 day before event start, 1 hour before event start
    /// 3: event created, 1 day before RSVP due date, 1 hour before RSVP due date, 2 days before event start, 1 day before event start, 1 hour before event start
    notify_amount: u32,
    responded: EventResponse,
}

#[derive(Clone, Deserialize, Serialize, Debug, Eq, PartialEq)]
pub enum EventResponse {
    Going,
    NotGoing,
    NoResponse
}

impl Guest {
    pub fn new(user_id: impl Into<String>, notify_amount: u32) -> Option<Self> {
        // Check if user is valid
        let user_id = user_id.into();

        if user_id.is_empty() {
            error!("Guest user is empty. Excluding from list.");
            return None;
        }

        if notify_amount > 3 {
            error!("Guest user {user_id} cannot have notify amount of {notify_amount}. Excluding from list.");
            return None;
        }

        Some(
            Self {
                user_id,
                notify_amount,
                responded: EventResponse::NoResponse,
            }
        )
    }

    pub fn user_id(&self) -> &str {
        &self.user_id
    }

    pub fn notify_amount(&self) -> u32 {
        self.notify_amount
    }

    pub fn responded(&self) -> EventResponse {
        self.responded.clone()
    }

    pub fn set_responded(&mut self, responded: EventResponse) {
        self.responded = responded;
    }
}

#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct ChangeRSVP {
    pub event_id: String,
    pub user_id: String,
    pub responded: EventResponse,
}

#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct Event {
    event_id: String,
    start_time: NaiveDateTime,
    end_time: NaiveDateTime,
    rsvp_due: NaiveDateTime,
    event_type: String,
    event_title: String,
    guest_list: Vec<Guest>,
    notify_threads_spawned: bool,
}

impl Event {
    pub fn event_id(&self) -> &str {
        &self.event_id
    }

    pub fn start_time(&self) -> NaiveDateTime {
        self.start_time
    }

    pub fn end_time(&self) -> NaiveDateTime {
        self.end_time
    }

    pub fn event_type(&self) -> &str {
        &self.event_type
    }

    pub fn event_title(&self) -> &str {
        &self.event_title
    }

    pub fn guest_list(&self) -> &Vec<Guest> {
        &self.guest_list
    }
    pub fn mut_guest_list(&mut self) -> &mut Vec<Guest> {
        &mut self.guest_list
    }

    pub fn notify_threads_spawned(&self) -> bool {
        self.notify_threads_spawned
    }

    pub fn set_notify_threads_spawned(&mut self, notify_threads_spawned: bool) {
        self.notify_threads_spawned = notify_threads_spawned;
    }

    pub fn rsvp_due(&self) -> NaiveDateTime {
        self.rsvp_due
    }
}

#[cfg(debug_assertions)]
impl Default for Event {
    fn default() -> Self {
        Self {
            event_id: "TEST_EVENT".to_string(),
            event_type: "test_event_type".to_string(),
            event_title: "test_event_title".to_string(),
            guest_list: vec![
                Guest::new("219587482528907264", 3).unwrap(),
                Guest::new("126161589291188224", 3).unwrap(),
            ],
            start_time: Local::now().naive_local(),
            rsvp_due: Local::now().naive_local().add(TimeDelta::minutes(62)),
            end_time: NaiveDateTime::default().add(TimeDelta::hours(2)),
            notify_threads_spawned: false,
        }
    }
}
