use anyhow::{Context, Result};
use bytes::Bytes;
use reqwest::blocking::Client;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct TelegramResponse<T> {
    pub ok: bool,
    pub result: T,
}

#[derive(Debug, Deserialize)]
pub struct Update {
    pub update_id: i32,
    pub message: Message,
}

#[derive(Debug, Deserialize)]
pub struct Message {
    #[serde(default)]
    pub from: Option<User>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub document: Option<Document>,
    pub chat: Chat,
}

#[derive(Debug, Deserialize)]
pub struct Chat {
    pub id: i64,
}

#[derive(Debug, Deserialize)]
pub struct Document {
    pub file_id: String,
    pub file_name: String,
    pub mime_type: String,
}

#[derive(Debug, Deserialize)]
pub struct User {
    pub id: i32,
    pub is_bot: bool,
    pub first_name: String,
    #[serde(default)]
    pub last_name: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
}
#[derive(Debug, Deserialize)]
pub struct File {
    pub file_id: String,
    pub file_unique_id: String,
    pub file_size: Option<i32>,
    pub file_path: String,
}

pub struct TelegramClient<'a> {
    token: String,
    http_client: &'a Client,
}

impl<'a> TelegramClient<'a> {
    const BASE_TELEGRAM_API_URL: &'static str = "https://api.telegram.org/bot";
    const BASE_FILE_TELEGRAM_API_URL: &'static str = "https://api.telegram.org/file/bot";

    fn api_url(&self, method: &str) -> String {
        format!(
            "{}{}/{}",
            TelegramClient::BASE_TELEGRAM_API_URL,
            self.token,
            method
        )
    }

    fn file_api_url(&self, path: &str) -> String {
        format!(
            "{}{}/{}",
            TelegramClient::BASE_FILE_TELEGRAM_API_URL,
            self.token,
            path
        )
    }

    pub fn new(token_value: String, http_client: &Client) -> TelegramClient {
        TelegramClient {
            token: token_value,
            http_client,
        }
    }

    pub fn get_updates(&self, update_id: i32) -> Result<TelegramResponse<Vec<Update>>> {
        self.http_client
            .get(&self.api_url(&format!("getUpdates?offset={:?}", update_id)))
            .send()
            .with_context(|| format!("Failed to receive updates from offset id {}", update_id))?
            .json()
            .with_context(|| {
                format!(
                    "Failed to parse response for getting updates with from the offset {}",
                    update_id
                )
            })
    }

    pub fn get_file(&self, file_id: &str) -> Result<TelegramResponse<File>> {
        self.http_client
            .get(&self.api_url(&format!("getFile?file_id={}", file_id)))
            .send()
            .with_context(|| format!("Failed to get file with id {}", file_id))?
            .json()
            .with_context(|| {
                format!(
                    "Failed to parse response for getting file with id {}",
                    file_id
                )
            })
    }

    pub fn send_message(&self, chat_id: i64, text: String) -> Result<()> {
        let mut body = std::collections::HashMap::<&str, String>::new();
        body.insert("chat_id", chat_id.to_string());
        body.insert("text", text.to_string());
        let json_body = serde_json::to_string(&body).with_context(|| {
            format!(
                "Failed to serialize body to json for sending message {:?}",
                body
            )
        });
        Ok(self
            .http_client
            .post(&self.api_url("sendMessage"))
            .body(json_body?)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .send()
            .with_context(|| {
                format!(
                    "Failed to send the message for chat id {} with text {}",
                    chat_id, text
                )
            })
            .map(|_| ())?)
    }

    pub fn donwload_file(&self, file_path: &str) -> Result<Bytes> {
        Ok(self
            .http_client
            .get(&self.file_api_url(file_path))
            .send()
            .with_context(|| {
                format!(
                    "Failed to send telegram api request for file download with path {}",
                    file_path
                )
            })?
            .bytes()
            .with_context(|| {
                format!(
                    "Failed to get bytes for file download request with path {}",
                    file_path
                )
            })?)
    }
}
