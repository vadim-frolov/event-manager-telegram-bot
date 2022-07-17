use r2d2_sqlite::SqliteConnectionManager;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;
use chrono::DateTime;

use teloxide::{
    prelude::*,
    types::{InlineKeyboardMarkup, ParseMode, UserId},
    RequestError,
};

pub type DbPool = r2d2::Pool<SqliteConnectionManager>;
//pub type EventId = u64;

#[derive(Deserialize, Serialize, Clone, Default, Debug)]
pub struct Configuration {
    pub telegram_bot_token: String,
    pub admin_ids: String,
    pub public_lists: bool,
    pub automatic_blacklisting: bool,
    pub drop_events_after_hours: u64,
    pub delete_from_black_list_after_days: u64,
    pub too_late_to_cancel_hours: u64,
    pub cleanup_old_events: bool,
    pub event_list_page_size: u64,
    pub event_page_size: u64,
    pub presence_page_size: u64,
    pub cancel_future_reservations_on_ban: bool,
    pub support: String,
    pub help: String,
    pub limit_bulk_notifications_per_second: u64,
    pub mailing_hours: String,
    pub mailing_hours_from: Option<u64>,
    pub mailing_hours_to: Option<u64>,
}

impl Configuration {
    pub fn parse(&mut self) -> Result<(), String> {
        let parts: Vec<&str> = self.mailing_hours.split('.').collect();
        if parts.len() != 3 {
            return Err("Wrong mailing hours format.".to_string());
        }
        match (
            DateTime::parse_from_str(&format!("2022-07-06 {}", parts[0]), "%Y-%m-%d %H:%M  %z"),
            DateTime::parse_from_str(&format!("2022-07-06 {}", parts[2]), "%Y-%m-%d %H:%M  %z"),
        ) {
            (Ok(from), Ok(to)) => {
                self.mailing_hours_from = Some((from.timestamp() % 86400) as u64);
                self.mailing_hours_to = Some((to.timestamp() % 86400) as u64);
                Ok(())
            }
            _ => Err("Failed to farse mailing hours.".to_string()),
        }
    }    
}

#[derive(Clone)]
pub struct Event {
    pub id: u64,
    pub name: String,
    pub link: String,
    pub max_adults: u64,
    pub max_children: u64,
    pub max_adults_per_reservation: u64,
    pub max_children_per_reservation: u64,
    pub ts: u64,
    pub remind: u64,
}

#[derive(PartialEq)]
pub enum EventState {
    Open,
    Closed,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct User {
    pub id: UserId,
    pub user_name1: String,
    pub user_name2: String,
    pub is_admin: bool,
}

impl User {
    pub fn new(u: &teloxide::types::User, admins: &HashSet<u64>) -> User {
        let mut user_name1 = u.first_name.clone();
        if let Some(v) = u.last_name.clone() {
            user_name1.push_str(" ");
            user_name1.push_str(&v);
        }
        let user_name2 = match u.username.clone() {
            Some(name) => name,
            None => "".to_string(),
        };

        User {
            id: u.id,
            user_name1,
            user_name2: user_name2.clone(),
            is_admin: admins.contains(&u.id.0),
        }
    }
}

pub struct Participant {
    pub user_id: u64,
    pub user_name1: String,
    pub user_name2: String,
    pub adults: u64,
    pub children: u64,
    pub attachment: Option<String>,
}

pub struct Presence {
    pub user_id: u64,
    pub user_name1: String,
    pub user_name2: String,
    pub reserved: u64,
    pub attachment: Option<String>,
}

pub struct MessageBatch {
    pub message_id: u64,
    pub event_id: u64,
    pub sender: String,
    pub message_type: MessageType,
    pub waiting_list: u64,
    pub text: String,
    pub recipients: Vec<u64>,
}

#[derive(FromPrimitive, ToPrimitive, PartialEq)]
pub enum MessageType {
    Direct = 0,
    Reminder = 1,
    WaitingListPrompt = 2,
}

//#[derive(Clone)]
pub struct Context {
    pub config: Configuration,
    pub pool: DbPool,
    pub sign_up_mutex: Arc<Mutex<u64>>,
    pub admins: HashSet<u64>,
}

#[derive(Debug)]
pub struct Reply {
    pub message: String,
    pub parse_mode: ParseMode,
    pub disable_preview: bool,
    pub reply_markup: Option<InlineKeyboardMarkup>,
}
impl Reply {
    pub fn new(message: String) -> Self {
        Self {
            message,
            parse_mode: ParseMode::Html,
            disable_preview: true,
            reply_markup: None,
        }
    }
    pub fn parse_mode(mut self, parse_mode: ParseMode) -> Self {
        self.parse_mode = parse_mode;
        self
    }

    pub fn reply_markup(mut self, reply_markup: InlineKeyboardMarkup) -> Self {
        self.reply_markup = Some(reply_markup);
        self
    }

    pub async fn send(self, msg: &Message, bot: &AutoSend<Bot>) -> Result<(), RequestError> {
        let fut = if let Some(reply_markup) = self.reply_markup {
            bot.send_message(msg.chat.id, self.message)
                .parse_mode(self.parse_mode)
                .disable_web_page_preview(self.disable_preview)
                .reply_markup(reply_markup)
        } else {
            bot.send_message(msg.chat.id, self.message)
                .parse_mode(self.parse_mode)
                .disable_web_page_preview(self.disable_preview)
        };
        fut.await.or_else(|e| {
            error!("Failed to send message to Telegram: {}", e);
            Err(e)
        })?;
        Ok(())
    }

    pub async fn edit(self, msg: &Message, bot: &AutoSend<Bot>) -> Result<(), RequestError> {
        let fut = if let Some(reply_markup) = self.reply_markup {
            bot.edit_message_text(msg.chat.id, msg.id, self.message)
                .parse_mode(self.parse_mode)
                .disable_web_page_preview(self.disable_preview)
                .reply_markup(reply_markup)
        } else {
            bot.edit_message_text(msg.chat.id, msg.id, self.message)
                .parse_mode(self.parse_mode)
                .disable_web_page_preview(self.disable_preview)
        };
        fut.await.or_else(|e| {
            error!("Failed to send message to Telegram: {}", e);
            Err(e)
        })?;
        Ok(())
    }
}
