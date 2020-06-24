use bytes::Bytes;
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

pub struct TelegramClient {
    token: String,
    http_client: reqwest::blocking::Client,
}

impl TelegramClient {
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

    pub fn new(token_value: String) -> TelegramClient {
        TelegramClient {
            token: token_value,
            http_client: reqwest::blocking::Client::new(),
        }
    }

    pub fn get_updates(&self, update_id: i32) -> reqwest::Result<TelegramResponse<Vec<Update>>> {
        self.http_client
            .get(&self.api_url(&format!("getUpdates?offset={:?}", update_id)))
            .send()?
            .json()
    }

    pub fn get_file(&self, file_id: &str) -> reqwest::Result<TelegramResponse<File>> {
        self.http_client
            .get(&self.api_url(&format!("getFile?file_id={}", file_id)))
            .send()?
            .json()
    }

    pub fn send_message(&self, chat_id: i64, text: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut body = std::collections::HashMap::<&str, String>::new();
        body.insert("chat_id", chat_id.to_string());
        body.insert("text", text.to_string());
        let json_body = serde_json::to_string(&body);
        Ok(self
            .http_client
            .post(&self.api_url("sendMessage"))
            .body(json_body?)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .send()
            .map(|_| ())?)
    }

    pub fn donwload_file<'a>(&self, file_path: &str) -> Result<Bytes, Box<dyn std::error::Error>> {
        Ok(self
            .http_client
            .get(&self.file_api_url(file_path))
            .send()?
            .bytes()?)
    }
}
