use bytes::Bytes;
use reqwest::blocking::get;
use serde::Deserialize;
use std::fmt;

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

#[derive(Debug)]
pub enum FileDownloadError {
    IoError(std::io::Error),
    HttpError(reqwest::Error),
}

impl From<std::io::Error> for FileDownloadError {
    fn from(err: std::io::Error) -> Self {
        FileDownloadError::IoError(err)
    }
}

impl From<reqwest::Error> for FileDownloadError {
    fn from(err: reqwest::Error) -> Self {
        FileDownloadError::HttpError(err)
    }
}

impl fmt::Display for FileDownloadError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let cause_error_msg = match self {
            FileDownloadError::IoError(e) => format!("{}", e),
            FileDownloadError::HttpError(e) => format!("{}", e),
        };
        write!(
            f,
            "Error while trying to process telegram client call {}",
            cause_error_msg
        )
    }
}

// This is important for other errors to wrap this one.
impl std::error::Error for FileDownloadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        // Generic error, underlying cause isn't tracked.
        match self {
            FileDownloadError::IoError(e) => Some(e),
            FileDownloadError::HttpError(e) => Some(e),
        }
    }
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
        get(&self.api_url(&format!("getUpdates?offset={:?}", update_id)))?.json()
    }

    pub fn get_file(&self, file_id: &str) -> reqwest::Result<TelegramResponse<File>> {
        get(&self.api_url(&format!("getFile?file_id={}", file_id)))?.json()
    }

    pub fn send_message(
        &self,
        chat_id: i64,
        text: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut body = std::collections::HashMap::<String, String>::new();
        body.insert("chat_id".to_string(), chat_id.to_string());
        body.insert("text".to_string(), text);
        let json_body = serde_json::to_string(&body);
        Ok(self.http_client
            .post(&self.api_url("sendMessage"))
            .body(json_body?)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .send()
            .map(|_| ())?)
    }

    pub fn donwload_file<'a>(&self, file_path: &str) -> Result<Bytes, FileDownloadError> {
        Ok(get(&self.file_api_url(file_path))?.bytes()?)
    }
}
