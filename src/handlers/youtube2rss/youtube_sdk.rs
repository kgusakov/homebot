use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;
use std::{collections::VecDeque, env::var};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Resp {
    pub items: VecDeque<Item>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Item {
    pub id: String,
    pub snippet: Snippet,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snippet {
    pub published_at: String,
    pub channel_id: String,
    pub title: String,
    pub description: String,
}

pub struct YoutubeSdk {
    http_client: Client,
    api_key: String,
}

impl YoutubeSdk {
    pub fn new() -> Self {
        let api_key =
            { var("GOOGLE_API_KEY").expect("Provide GOOGLE_API_KEY environment variable please") };
        Self {
            http_client: Client::new(),
            api_key,
        }
    }

    pub async fn get_video_info(&self, video_id: &str) -> Result<Option<Snippet>> {
        let url = format!(
            "https://www.googleapis.com/youtube/v3/videos?part=snippet&id={}&key={}",
            video_id, self.api_key
        );
        let mut resp: Resp = self
            .http_client
            .get(&url)
            .send()
            .await
            .with_context(|| {
                format!(
                    "Failed to send request for getting video info with id {}",
                    video_id
                )
            })?
            .json()
            .await
            .with_context(|| {
                format!(
                    "Failed to deserialize result of video info request with id {}",
                    video_id
                )
            })?;
        Ok(resp.items.pop_front().map(|i| i.snippet))
    }
}
