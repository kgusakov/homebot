use crate::{
    telegram_api::{Message, SendMessage, TelegramClient},
    Handler, HandlerContext,
};

use anyhow::Result;

pub struct HealthCheckHandler<'a> {
    telegram_client: &'a TelegramClient<'a>,
}

impl<'a> Handler for HealthCheckHandler<'a> {
    fn name(&self) -> String {
        String::from("HealthCheck")
    }

    fn process(&self, m: &Message) -> Result<()> {
        match &m.text {
            Some(t) if t.starts_with("ping") => {
                Ok(self.telegram_client.send_message(SendMessage {
                    chat_id: m.chat.id.to_string(),
                    text: String::from("pong"),
                    reply_to_message_id: Some(&m.message_id),
                })?)
            }
            _ => Ok(()),
        }
    }
}

impl<'a> HealthCheckHandler<'a> {
    pub fn new(handler_context: &'a HandlerContext) -> Self {
        Self {
            telegram_client: handler_context.telegram_client,
        }
    }
}
