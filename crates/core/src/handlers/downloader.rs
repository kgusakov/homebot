use std::{
    env::{self, temp_dir},
    fs::remove_dir_all,
    path::{Path, PathBuf},
    process::Output,
};

use async_trait::async_trait;
use shlex::Shlex;
use telegram_api::Message;
use tokio::{fs::create_dir, process::Command};

use crate::{HandlerContext, TelegramClient, handlers::AsyncHandler};

use anyhow::anyhow;
use anyhow::{Context, Result};

const INSTAGRAM_URL_START: &str = "https://www.instagram.com/reel/";
const YT_URL_CONTAINS: &str = "youtube.com/shorts/";

pub struct DownloaderHandler<'a> {
    telegram_client: &'a TelegramClient<'a>,
    tmp_dir: PathBuf,
    socks_proxy_url: String,
    yt_dlp_path: PathBuf,
    yt_dlp_opts: Vec<String>,
    cookies_path: PathBuf,
}

#[async_trait]
impl<'a> AsyncHandler for DownloaderHandler<'a> {
    fn name(&self) -> String {
        String::from("Downloader")
    }

    async fn process(&self, m: &Message) -> Result<()> {
        match &m.text {
            Some(t) if t.starts_with(INSTAGRAM_URL_START) || t.contains(YT_URL_CONTAINS) => {
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

        let yt_dlp_opts =
            Shlex::new(&env::var("DOWNLOADER_YT_DLP_OPTS").unwrap_or(String::from("")))
                .collect::<Vec<String>>();

        let cookies_path = PathBuf::from(
            env::var("DOWNLOADER_COOKIES_PATH")
                .expect("Provide DOWNLOADER_COOKIES_PATH environment variable please"),
        );

        let tmp_dir = temp_dir();

        Self {
            telegram_client: handler_context.telegram_client,
            tmp_dir,
            socks_proxy_url,
            yt_dlp_path,
            yt_dlp_opts,
            cookies_path,
        }
    }

    async fn process_url(&self, chat_id: &str, message_id: &i64, url: &str) -> Result<()> {
        let message_download_tmp_dir = self.tmp_dir.join(format!("tmp_{}", message_id));

        create_dir(message_download_tmp_dir.as_path()).await?;

        let download_path = message_download_tmp_dir.join(format!("%(id)s.%(ext)s",));

        let downloaded_file_path: PathBuf = String::from_utf8(
            self.download(url, &download_path)
                .await
                .with_context(|| "Can't download video to send via telegram")?
                .stdout,
        )
        .with_context(|| "Can't stringify download path")?
        .trim()
        .into();

        self.telegram_client
            .async_send_file(chat_id, downloaded_file_path)
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
            .args(&["--print", "after_move:filepath"])
            .args(&[
                "--cookies",
                self.cookies_path
                    .as_path()
                    .to_str()
                    .ok_or(anyhow!("Can't convert path to string"))?,
            ])
            .args(&self.yt_dlp_opts)
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
}
