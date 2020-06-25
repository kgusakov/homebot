use crate::{
    telegram_api::{Message, TelegramClient},
    Handler,
};

use anyhow::Result;

pub struct HealthCheckHandler<'a> {
    telegram_client: &'a TelegramClient,
}

impl<'a> Handler for HealthCheckHandler<'a> {
    fn name(&self) -> String {
        String::from("HealthCheck")
    }

    fn process(&self, m: &Message) -> Result<()> {
        match &m.text {
            Some(t) if t.starts_with("ping") => {
                Ok(self.telegram_client.send_message(m.chat.id, "pong")?)
            }
            _ => Ok(()),
        }
    }
}

impl<'a> HealthCheckHandler<'a> {
    pub fn new(telegram_client: &'a TelegramClient) -> Self {
        Self { telegram_client }
    }
}
