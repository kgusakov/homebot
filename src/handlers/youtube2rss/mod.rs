mod metadata;
mod s3_storage;
mod youtube_sdk;

use super::AsyncHandler;
use crate::{HandlerContext, Message, SendMessage, TelegramClient, User};
use s3_storage::S3Storage;
use std::{
    collections::VecDeque, env, env::temp_dir, fs, path::PathBuf, process::Output, time::SystemTime,
};
use youtube_sdk::YoutubeSdk;

use metadata::*;

use regex::Regex;

use rss::ChannelBuilder;
use rss::{Enclosure, Item as RItem};

use chrono::offset::Utc;
use chrono::DateTime;

use anyhow::anyhow;
use anyhow::{Context, Result};

use async_trait::async_trait;

use tokio::sync::Mutex;

use tokio::process::Command;

fn metadata_path(user: &str) -> String {
    format!("{}/metadata.mp", user)
}

fn data_path(user: &str) -> String {
    format!("{}/audio", user)
}

fn rss_path(user: &str) -> String {
    format!("{}/feed.xml", user)
}

pub struct PodcastHandler<'a> {
    youtube_extractor: String,
    youtube_sdk: YoutubeSdk,
    id_regex: Regex,
    tmp_dir: PathBuf,
    s3_client: S3Storage,
    metadata: Mutex<MetadataStorage>,
    telegram_client: &'a TelegramClient<'a>,
}

impl<'a> PodcastHandler<'a> {
    pub fn new(handler_context: &'a HandlerContext) -> Self {
        let youtube_extractor = {
            env::var("YOUTUBE_EXTRACTOR")
                .expect("Provide YOUTUBE_EXTRACTOR environment variable please")
        };
        let tmp_dir = temp_dir();

        Self {
            youtube_extractor,
            youtube_sdk: YoutubeSdk::new(),
            id_regex: Regex::new(r"(v=|youtu.be/)(?P<id>[^&]*)")
                .expect("Failed to compile video id Regex"),
            tmp_dir,
            s3_client: S3Storage::new(),
            metadata: Mutex::new(MetadataStorage::new()),
            telegram_client: handler_context.telegram_client,
        }
    }

    async fn process_url(&self, url: &str, user: Option<&User>, message_id: i64) -> Result<String> {
        let username = &user
            .ok_or(anyhow!(
                "Empty user of message. Can't manage podcasts for empty user"
            ))?
            .first_name;
        let video_id = self.extract_id(url)?;
        let download_path = self
            .tmp_dir
            .join(format!("{}{}", message_id, "%(id)s.%(ext)s"))
            .to_str()
            .expect("Failed to convert to string file path of mp3 file")
            .to_string();
        self.download(url, &download_path).await?;

        let downloaded_file_path = self.tmp_dir.join(format!("{}{}.mp3", message_id, video_id));
        let s3_result_file_path = format!("{}/{}.mp3", data_path(&username), &video_id);
        self.s3_client
            .upload_file(
                downloaded_file_path.to_path_buf(),
                s3_result_file_path.to_string(),
            )
            .await?;

        let file_size = {
            let metadata = fs::metadata(
                downloaded_file_path
                    .to_str()
                    .ok_or(anyhow!("Failed to lossy convert file path"))?,
            )?;
            metadata.len()
        };
        if let Some(video_info) = self.youtube_sdk.get_video_info(&video_id).await? {
            let video_metadata = VideoMetadata {
                file_size,
                file_url: self.s3_client.get_public_url(&s3_result_file_path),
                video_id: video_id.to_string(),
                created_at: SystemTime::now(),
                name: video_info.title,
                original_link: url.to_string(),
            };

            {
                let metadta_storage = self.metadata.lock().await;
                let mut metadata = metadta_storage
                    .load_metadata(&metadata_path(&username))
                    .await?;
                metadata.push_front(video_metadata);
                metadta_storage
                    .update_metadata(&metadata_path(&username), &metadata)
                    .await?;

                let rss = Self::generate_rss(&username, &metadata)?;
                self.s3_client
                    .upload_object(rss.into_bytes(), &rss_path(&username))
                    .await?;
            };

            Ok(self.s3_client.get_public_url(&rss_path(&username)))
        } else {
            Err(anyhow!("Received empty video info about {}", url))
        }
    }

    fn generate_rss(user: &str, metadata: &VecDeque<VideoMetadata>) -> Result<String> {
        let mut items = vec![];
        for item in metadata {
            let pub_date: DateTime<Utc> = item.created_at.into();
            let mut ritem = RItem::default();
            ritem.set_title(item.name.to_string());
            ritem.set_pub_date(pub_date.to_rfc2822());
            let mut enc = Enclosure::default();
            enc.set_mime_type("audio/mp3");
            enc.set_url(item.file_url.to_string());
            enc.set_length(item.file_size.to_string());
            ritem.set_enclosure(enc);
            items.push(ritem)
        }

        let channel = ChannelBuilder::default()
            .title(format!("Куточок {}", user))
            .items(items)
            .build()
            .map_err(|e| {
                anyhow!(
                    "Failed to build rss channel for user {} with error {}",
                    user,
                    e
                )
            })?;
        Ok(channel.to_string())
    }

    async fn download(&self, url: &str, path: &str) -> Result<Output> {
        let res = Command::new(&self.youtube_extractor)
            .env("https_proxy", "")
            .arg("-x")
            .args(&["--audio-format", "mp3"])
            .args(&["-o", path])
            .arg(url)
            .output()
            .await
            .with_context(|| {
                format!(
                    "Failed to execute the youtube-dl command to download url {}",
                    url
                )
            })?;
        if res.status.success() {
            Ok(res)
        } else {
            Err(anyhow!(
                "Exit code of youtube-dl command was not 0, output: {:?}",
                res
            ))
        }
    }

    fn extract_id(&self, s: &str) -> Result<String> {
        self.id_regex
            .captures(s)
            .and_then(|cap| {
                if let Some(id) = cap.name("id") {
                    Some(id.as_str().to_string())
                } else {
                    None
                }
            })
            .ok_or(anyhow!("Can't parse video id from youtube url {}", s))
    }
}

#[async_trait]
impl<'a> AsyncHandler for PodcastHandler<'a> {
    fn name(&self) -> String {
        String::from("Youtube2Rss")
    }

    async fn process(&self, m: &Message) -> Result<()> {
        match &m.text {
            Some(s)
                if s.starts_with("https://www.youtube.com/watch")
                    || s.starts_with("https://youtu.be/") =>
            {
                let rss_feed_url = self.process_url(s, m.from.as_ref(), m.message_id).await?;
                Ok(self
                    .telegram_client
                    .async_send_message(SendMessage {
                        chat_id: m.chat.id.to_string(),
                        text: format!(
                            "RSS фид успешно обновлен и доступен по адресу: {}",
                            rss_feed_url
                        ),
                        reply_to_message_id: Some(&m.message_id),
                    })
                    .await?)
            }
            _ => Ok(()),
        }
    }
}
