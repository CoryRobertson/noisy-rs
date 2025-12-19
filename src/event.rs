use chrono::{NaiveDateTime, TimeDelta};
use serde::{Deserialize, Serialize};
use std::ops::Add;

#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct Guest {
    user_id: String,
    notify_amount: u32,
}

impl Guest {
    pub fn new(user_id: impl Into<String>, notify_amount: u32) -> Self {
        // TODO(costi): ???
        debug_assert!(notify_amount <= 3);

        Self {
            user_id: user_id.into(),
            notify_amount,
        }
    }
}

#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct Event {
    event_id: String,
    start_time: NaiveDateTime,
    end_time: NaiveDateTime,
    event_type: String,
    event_title: String,
    guest_list: Vec<Guest>,
}

#[cfg(debug_assertions)]
impl Default for Event {
    fn default() -> Self {
        Self {
            event_id: "TEST_EVENT".to_string(),
            event_type: "test_event_type".to_string(),
            event_title: "test_event_title".to_string(),
            guest_list: vec![
                Guest::new("Test person", 1),
                Guest::new("Test person 2", 2),
                Guest::new("Test person 3", 3),
                Guest::new("Test person 4", 0),
            ],
            start_time: NaiveDateTime::default(),
            end_time: NaiveDateTime::default().add(TimeDelta::hours(2)),
        }
    }
}
