use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use telegram_api::{Message, TelegramClient};

pub trait Handler {
    fn name(&self) -> String;

    fn process(&self, m: &Message) -> Result<()>;
}

#[async_trait]
pub trait AsyncHandler {
    fn name(&self) -> String;

    async fn process(&self, m: &Message) -> Result<()>;
}

pub struct HandlerContext<'a> {
    pub telegram_client: &'a TelegramClient<'a>,
    pub async_http_client: &'a Client,
    pub async_proxy_http_client: &'a Client,
}
