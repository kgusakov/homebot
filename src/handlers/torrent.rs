use crate::telegram_api::*;
use base64::encode;
use serde::{Deserialize, Serialize};
use std::env;

pub struct TorrentFilter<'a> {
    telegram_client: &'a TelegramClient,
    transmission_client: TransmissionClient,
}

impl<'a> crate::Filter for TorrentFilter<'a> {
    fn process(&self, message: &Message) -> Result<(), Box<dyn std::error::Error>> {
        let process_success = |r: Response| match r {
            Response {
                arguments: ResponseArguments::TorerntAdded { name: n, .. },
                ..
            } => self
                .telegram_client
                .send_message(message.chat.id, &format!("{} успешно добавлен", n)),
            Response {
                arguments: ResponseArguments::TorerntDuplicate { name: n, .. },
                ..
            } => self
                .telegram_client
                .send_message(message.chat.id, &format!("{} уже был добавлен ранее", n)),
        };

        match message {
            Message {
                document: Some(doc),
                ..
            } if doc.file_name.ends_with(".torrent") => self
                .process_torrent(&doc.file_id)
                .and_then(process_success),
            _ => Ok(()),
        }
    }
}

impl<'a> TorrentFilter<'a> {
    pub fn new(telegram_client: &'a TelegramClient) -> Self {
        Self {
            telegram_client,
            transmission_client: TransmissionClient::new(),
        }
    }

    fn process_torrent(&self, file_id: &str) -> RequestResult<Response> {
        let response = self.telegram_client.get_file(file_id)?;
        let content = self
            .telegram_client
            .donwload_file(&response.result.file_path)?;
        self.transmission_client.torrent_add(&content.to_vec())
    }
}

struct TransmissionClient {
    transmission_address: String,
    http_client: reqwest::blocking::Client,
}

#[derive(Serialize, Debug)]
#[serde(untagged)]
enum RequestArguments {
    TorrentAdd {
        #[serde(skip_serializing_if = "Option::is_none")]
        filename: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        metainfo: Option<String>,
    },
}
#[derive(Serialize, Debug)]
struct Request {
    method: String,
    arguments: RequestArguments,
}

#[derive(Deserialize, Debug)]
struct Response {
    result: String,
    arguments: ResponseArguments,
}

#[derive(Deserialize, Debug)]
enum ResponseArguments {
    #[serde(rename = "torrent-duplicate")]
    TorerntDuplicate { id: i32, name: String },

    #[serde(rename = "torrent-added")]
    TorerntAdded { id: i32, name: String },
}

type RequestResult<T> = Result<T, Box<dyn std::error::Error>>;

impl TransmissionClient {
    fn new() -> Self {
        let transmission_address = {
            env::var("TRANSMISSION_ADDRESS")
                .expect("Provide TRANSMISSION_ADDRESS environment variable please")
        };
        Self {
            transmission_address: transmission_address,
            http_client: reqwest::blocking::Client::new(),
        }
    }

    fn req_with_sessions_id_loop<T: serde::de::DeserializeOwned>(
        &self,
        request: Request,
    ) -> RequestResult<T> {
        let resp: Result<reqwest::blocking::Response, Box<dyn std::error::Error>> = match self
            .http_client
            .post(&self.transmission_address)
            .body(serde_json::to_string(&request)?)
            .send()
        {
            Ok(r) if r.status() == reqwest::StatusCode::CONFLICT => {
                if let Some(session_id) = r.headers().get("X-Transmission-Session-Id") {
                    Ok(self.http_client
                        .post(&self.transmission_address)
                        .body(serde_json::to_string(&request)?)
                        .header("X-Transmission-Session-Id", session_id.to_str()?)
                        .send()?)
                } else {
                    Ok(r)
                }
            }
            r => Ok(r?)
        };
        Ok(resp?.json()?)
    }

    pub fn torrent_add(&self, file_content: &[u8]) -> RequestResult<Response> {
        let base64_encoded = encode(file_content);
        let request = Request {
            method: "torrent-add".to_string(),
            arguments: RequestArguments::TorrentAdd {
                filename: None,
                metainfo: Some(base64_encoded),
            },
        };

        self.req_with_sessions_id_loop(request)
    }
}

#[test]
fn test_torrent_add_by_file_path() {
    env::set_var(
        "TRANSMISSION_ADDRESS",
        "http://192.168.1.104:9091/transmission/rpc",
    );
    let client = TransmissionClient::new();

    let request = Request {
        method: "torrent-add".to_string(),
        arguments: RequestArguments::TorrentAdd {
            filename: Some("/home/kgusakov/tt.torrent".to_string()),
            metainfo: None,
        },
    };
    let r = client.req_with_sessions_id_loop::<Response>(request);
    assert!(r.is_ok())
}

#[test]
fn test_torrent_add_by_metainfo() {
    env::set_var(
        "TRANSMISSION_ADDRESS",
        "http://192.168.1.104:9091/transmission/rpc",
    );
    let client = TransmissionClient::new();

    let file_content = fs::read("/Users/kirill/tt.torrent").unwrap();
    assert!(client.torrent_add(&file_content).is_ok());
}
