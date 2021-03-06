use super::AsyncHandler;
use crate::telegram_api::*;
use crate::HandlerContext;
use anyhow::{Context, Result};
use async_trait::async_trait;
use base64::encode;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;

pub struct TorrentHandler<'a> {
    telegram_client: &'a TelegramClient<'a>,
    transmission_client: TransmissionClient<'a>,
}

#[async_trait]
impl<'a> AsyncHandler for TorrentHandler<'a> {
    fn name(&self) -> String {
        String::from("TransmissionClient")
    }

    async fn process(&self, message: &Message) -> Result<()> {
        let process_success = |r: Response| async move {
            match r {
                Response {
                    arguments: ResponseArguments::TorerntAdded { name: n, .. },
                    ..
                } => {
                    self.telegram_client
                        .async_send_message(SendMessage {
                            chat_id: message.chat.id.to_string(),
                            text: format!("{} успешно добавлен", n),
                            reply_to_message_id: Some(&message.message_id),
                        })
                        .await
                }
                Response {
                    arguments: ResponseArguments::TorerntDuplicate { name: n, .. },
                    ..
                } => {
                    self.telegram_client
                        .async_send_message(SendMessage {
                            chat_id: message.chat.id.to_string(),
                            text: format!("{} уже был добавлен ранее", n),
                            reply_to_message_id: Some(&message.message_id),
                        })
                        .await
                }
            }
        };

        match message {
            Message {
                document: Some(doc),
                ..
            } if doc.file_name.ends_with(".torrent") => {
                process_success(self.process_torrent(&doc.file_id).await?).await
            }
            _ => Ok(()),
        }
    }
}

impl<'a> TorrentHandler<'a> {
    pub fn new(handler_context: &'a HandlerContext) -> Self {
        Self {
            telegram_client: handler_context.telegram_client,
            transmission_client: TransmissionClient::new(handler_context.async_http_client),
        }
    }

    async fn process_torrent(&self, file_id: &str) -> Result<Response> {
        let response = self.telegram_client.async_get_file(file_id).await?;
        let content = self
            .telegram_client
            .async_donwload_file(&response.result.file_path)
            .await?;
        self.transmission_client
            .torrent_add(&content.to_vec())
            .await
    }
}

struct TransmissionClient<'a> {
    transmission_address: String,
    http_client: &'a Client,
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

impl<'a> TransmissionClient<'a> {
    fn new(http_client: &'a Client) -> Self {
        let transmission_address = {
            env::var("TRANSMISSION_ADDRESS")
                .expect("Provide TRANSMISSION_ADDRESS environment variable please")
        };
        Self {
            transmission_address: transmission_address,
            http_client: http_client,
        }
    }

    async fn req_with_sessions_id_loop<T: serde::de::DeserializeOwned>(
        &self,
        request: Request,
    ) -> Result<T> {
        // TODO: success session id should be persisted
        let first_try_resp: Result<reqwest::Response> = self
            .http_client
            .post(&self.transmission_address)
            .body(
                serde_json::to_string(&request)
                    .with_context(|| format!("Failed to serialize request {:?}", request))?,
            )
            .send()
            .await
            .with_context(|| {
                format!(
                    "Failed to send http post request to transmission api {:?}",
                    request
                )
            });
        let result = match first_try_resp {
            Ok(r) if r.status() == reqwest::StatusCode::CONFLICT => {
                if let Some(session_id) = r.headers().get("X-Transmission-Session-Id") {
                    Ok(self
                        .http_client
                        .post(&self.transmission_address)
                        .body(serde_json::to_string(&request)?)
                        .header("X-Transmission-Session-Id", session_id.to_str()?)
                        .send()
                        .await
                        .with_context(|| format!("Failed to send http post request to transmission api with correct session-id {:?}", request))?)
                } else {
                    Ok(r)
                }
            }
            r => r,
        };
        Ok(result?
            .json()
            .await
            .with_context(|| format!("Failed to parse result for request {:?}", request))?)
    }

    pub async fn torrent_add(&self, file_content: &[u8]) -> Result<Response> {
        let base64_encoded = encode(file_content);
        let request = Request {
            method: "torrent-add".to_string(),
            arguments: RequestArguments::TorrentAdd {
                filename: None,
                metainfo: Some(base64_encoded),
            },
        };

        self.req_with_sessions_id_loop(request).await
    }
}
