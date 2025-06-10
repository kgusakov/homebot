use std::{
    env::{self, temp_dir},
    fs::remove_dir_all,
    path::{Path, PathBuf},
    process::Output,
};

use async_trait::async_trait;
use regex::Regex;
use tokio::{fs::create_dir, process::Command};

use crate::{handlers::AsyncHandler, HandlerContext, Message, TelegramClient};

use anyhow::anyhow;
use anyhow::{Context, Result};

const INSTAGRAM_URL_START: &str = "https://www.instagram.com/reel/";
const YT_URL_START: &str = "https://www.youtube.com/shorts/";

pub struct DownloaderHandler<'a> {
    telegram_client: &'a TelegramClient<'a>,
    inst_id_regex: Regex,
    yt_id_regex: Regex,
    tmp_dir: PathBuf,
    socks_proxy_url: String,
    yt_dlp_path: PathBuf,
    cookies_path: PathBuf,
}

#[async_trait]
impl<'a> AsyncHandler for DownloaderHandler<'a> {
    fn name(&self) -> String {
        String::from("Downloader")
    }

    async fn process(&self, m: &Message) -> Result<()> {
        match &m.text {
            Some(t) if t.starts_with(INSTAGRAM_URL_START) || t.starts_with(YT_URL_START) => {
                self.process_url(m.chat.id.to_string().as_str(), &m.message_id, t)
                    .await
            }
            _ => Ok(()),
        }
    }
}

impl<'a> DownloaderHandler<'a> {
    pub fn new(handler_context: &'a HandlerContext) -> Self {
        // TODO: support processing without proxy
        let socks_proxy_url = env::var("DOWNLOADER_SOCKS_PROXY")
            .expect("Provide DOWNLOADER_SOCKS_PROXY environment variable please");

        let yt_dlp_path = PathBuf::from(
            env::var("DOWNLOADER_YT_DLP_PATH")
                .expect("Provide DOWNLOADER_YT_DLP_PATH environment variable please"),
        );

        let cookies_path = PathBuf::from(
            env::var("DOWNLOADER_COOKIES_PATH")
                .expect("Provide DOWNLOADER_COOKIES_PATH environment variable please"),
        );

        let tmp_dir = temp_dir();

        Self {
            telegram_client: handler_context.telegram_client,
            inst_id_regex: Regex::new(r"(v=|reel/)(?P<id>[^/]*)")
                .expect("Failed to compile video id Regex"),
            yt_id_regex: Regex::new(r"(v=|shorts/)(?P<id>[^/]*)")
                .expect("Failed to compile video id Regex"),
            tmp_dir,
            socks_proxy_url,
            yt_dlp_path,
            cookies_path,
        }
    }

    async fn process_url(&self, chat_id: &str, message_id: &i64, url: &str) -> Result<()> {
        let video_id = self.extract_id(url)?;

        let message_download_tmp_dir = self.tmp_dir.join(format!("tmp_{}", message_id));

        create_dir(message_download_tmp_dir.as_path()).await?;

        let download_path = message_download_tmp_dir
            // TODO support non-mp4 output
            .join(format!("{}.mp4", video_id));

        self.download(url, download_path.as_path()).await?;

        self.telegram_client
            .async_send_file(chat_id, download_path)
            .await?;

        remove_dir_all(message_download_tmp_dir)?;

        Ok(())
    }

    async fn download(&self, url: &str, path: &Path) -> Result<Output> {
        let res = Command::new(&self.yt_dlp_path)
            .args(&[
                "-o",
                path.to_str().expect("Failed to convert path to string"),
            ])
            .args(&["--proxy", self.socks_proxy_url.as_str()])
            .args(&[
                "--cookies",
                self.cookies_path
                    .as_path()
                    .to_str()
                    .ok_or(anyhow!("Can't convert path to string"))?,
            ])
            .arg(url)
            .output()
            .await
            .with_context(|| {
                format!(
                    "Failed to execute the yt-dlp command to download url {}",
                    url
                )
            })?;
        if res.status.success() {
            Ok(res)
        } else {
            Err(anyhow!(
                "Exit code of yt-dlp command was not 0, output: {:?}",
                res
            ))
        }
    }

    fn extract_id(&self, s: &str) -> Result<String> {
        let regex = match s {
            _ if s.starts_with(INSTAGRAM_URL_START) => Some(&self.inst_id_regex),
            _ if s.starts_with(YT_URL_START) => Some(&self.yt_id_regex),
            _ => None,
        }
        .ok_or(anyhow!("Didn't find id regex for url {}", s));

        regex?
            .captures(s)
            .and_then(|cap| {
                if let Some(id) = cap.name("id") {
                    Some(id.as_str().to_string())
                } else {
                    None
                }
            })
            .ok_or(anyhow!("Can't parse video id from url {}", s))
    }
}
