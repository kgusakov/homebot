use crate::{
  telegram_api::{TelegramClient, Message},
  Filter, ProcessingResult};

pub struct HealthCheckFilter<'a> {
  telegram_client: &'a TelegramClient,
}

impl<'a> Filter for HealthCheckFilter<'a> {
  fn process(&self, m: &Message) -> ProcessingResult {
    match &m.text {
      Some(t) if t.starts_with("ping") =>
        Ok(self.telegram_client.send_message(m.chat.id, "pong")?),
      _ => Ok(())
    }
  }
}

impl<'a> HealthCheckFilter<'a> {
  pub fn new(telegram_client: &'a TelegramClient) -> Self {
    Self { telegram_client }
  }
}