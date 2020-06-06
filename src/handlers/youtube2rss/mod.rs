mod youtube_sdk;
mod metadata;
mod s3_storage;

use s3_storage::S3Storage;
use std::{
  path::Path,
  env,
  process::{Command, Output},
  error,
  fmt,
  time::SystemTime,
  fs,
  collections::VecDeque
};
use youtube_sdk::YoutubeSdk;
use crate::{BResult, Filter, ProcessingResult, Message, User, TelegramClient};

use  metadata::*;

use regex::Regex;

use rss::ChannelBuilder;
use rss::{Item as RItem, Enclosure};

use chrono::offset::Utc;
use chrono::DateTime;


fn metadata_path(user: &str) -> String {
  format!("{}/metadata.mp", user)
}

fn data_path(user: &str) -> String {
  format!("{}/audio", user)
}

fn rss_path(user: &str) -> String {
  format!("{}/feed.xml", user)
}

pub struct PodcastFilter<'a> {
  youtube_extractor: String,
  youtube_sdk: YoutubeSdk,
  id_regex: Regex,
  tmp_dir: String,
  s3_client: S3Storage,
  metadata: Metadata,
  telegram_client: &'a TelegramClient,
}
#[derive(Debug, Clone)]
struct EmptyUserError;

#[derive(Debug, Clone)]
struct EmptyVideoInfo;
#[derive(Debug, Clone)]
struct FilePathConvertingError;
#[derive(Debug, Clone)]
struct CommandError {
  output: Output
}

impl fmt::Display for CommandError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
      write!(f, "{:?}", self.output)
  }
}

impl error::Error for CommandError {
  fn source(&self) -> Option<&(dyn error::Error + 'static)> {
      None
  }
}

impl fmt::Display for EmptyUserError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
      write!(f, "Empty user of message. Can't manage podcasts for empty user")
  }
}

impl error::Error for EmptyUserError {
  fn source(&self) -> Option<&(dyn error::Error + 'static)> {
      None
  }
}

impl fmt::Display for EmptyVideoInfo {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
      write!(f, "Empty user of message. Can't manage podcasts for empty user")
  }
}

impl error::Error for EmptyVideoInfo {
  fn source(&self) -> Option<&(dyn error::Error + 'static)> {
      None
  }
}

impl fmt::Display for FilePathConvertingError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
      write!(f, "Can't convert file path to string lossless")
  }
}

impl error::Error for FilePathConvertingError {
  fn source(&self) -> Option<&(dyn error::Error + 'static)> {
      None
  }
}

impl<'a> PodcastFilter<'a> {

  pub fn new(telegram_client: &'a TelegramClient) -> Self {
    let youtube_extractor = {
      env::var("YOUTUBE_EXTRACTOR")
          .expect("Provide YOUTUBE_EXTRACTOR environment variable please")
    };
    let tmp_dir = {
      env::var("BOT_TMP_DIR")
        .expect("Provide BOT_TMP_DIR environment variable please")
    };

    Self { youtube_extractor,
          youtube_sdk: YoutubeSdk::new(),
          id_regex: Regex::new(r"(v=|youtu.be/)(?P<id>[^&]*)").expect("Can't compile video id Regex"),
          tmp_dir,
          s3_client: S3Storage::new(),
          metadata: Metadata::new(),
          telegram_client
    }
  }

  fn process_url(&self, url: &str, user: Option<&User>) -> Result<String, Box<dyn std::error::Error>> {
    let username = &user.ok_or(EmptyUserError)?.first_name;
    let video_id = self.extract_id(url)?;
    let download_path = Path::new(&self.tmp_dir).join("%(id)s.%(ext)s")
      .to_str().expect("Can't convert to string file path of mp3 file").to_string();
    self.download(url, &download_path)?;
    let downloaded_file_path = Path::new(&self.tmp_dir).join(format!("{}.mp3", video_id));
    let s3_result_file_path = format!("{}/{}.mp3", data_path(&username), &video_id);
    self.s3_client.upload_file(&downloaded_file_path, &s3_result_file_path)?;
    
    let file_size = {
      let metadata = fs::metadata(downloaded_file_path.to_str().ok_or(FilePathConvertingError)?)?;
      metadata.len()
    };
    if let Some(video_info) = self.youtube_sdk.get_video_info(&video_id)? {
      let video_metadata = VideoMetadata {
        file_size,
        file_url: self.s3_client.get_public_url(&s3_result_file_path),
        video_id: video_id.to_string(),
        created_at: SystemTime::now(),
        name: video_info.title,
        original_link: url.to_string(),
      };
      let mut metadata =
        self.metadata.load_metadata(&metadata_path(&username))?;
      metadata.push_front(video_metadata);
      self.metadata.update_metadata(&metadata_path(&username), &metadata)?;

      let rss = Self::generate_rss(&username, &metadata)?;
      self.s3_client.upload_object(rss.into_bytes(), &rss_path(&username))?;

      Ok(self.s3_client.get_public_url(&rss_path(&username)))
    } else {
      Err(EmptyVideoInfo.into())
    }
  }

  fn generate_rss(user: &str, metadata: &VecDeque<VideoMetadata>) -> Result<String, Box<dyn std::error::Error>> {
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
        .build()?;
    Ok(channel.to_string())
  }

  fn download(&self, url: &str, path: &str) -> BResult<Output> {
    let res = Command::new(&self.youtube_extractor)
      .env("https_proxy", "")
      .arg("-x")
      .args(&["--audio-format", "mp3"])
      .args(&["-o", path])
      .arg(url)
      .output()?;
    if res.status.success() {
      Ok(res)
    } else {
      Err(CommandError {output: res}.into())
    }
  }

  fn extract_id(&self, s: &str) -> Result<String, Box<dyn std::error::Error>> {
    self.id_regex.captures(s).and_then(|cap| {
      if let Some(id) = cap.name("id")  {
        Some(id.as_str().to_string()) 
      } else {
        None
      }
    }).ok_or("Can't parse video id from youtube url".into())
  }
}


impl<'a> Filter for PodcastFilter<'a> {
  fn process(&self, m: &Message) -> ProcessingResult {
    match &m.text {
      Some(s) if s.starts_with("https://www.youtube.com/watch") || s.starts_with("https://youtu.be/")  => {
        let rss_feed_url = self.process_url(s, m.from.as_ref())?;
        Ok(self.telegram_client.send_message(m.chat.id, &rss_feed_url)?)
      },
      _ => Ok(())
    }
  }
}

#[test]
fn process_url_test() {
  let telegram_client: TelegramClient = {
    let token =
        env::var("TELEGRAM_TOKEN").expect("Provide TELEGRAM_TOKEN environment variable please");
    crate::telegram_api::TelegramClient::new(token)
};
  let filter = PodcastFilter::new(&telegram_client);
  // pub id: i32,
  //   pub is_bot: bool,
  //   pub first_name: String,
  //   #[serde(default)]
  //   pub last_name: Option<String>,
  //   #[serde(default)]
  //   pub username: Option<String>,
  let  user = User {
    id: 125504090,
    is_bot: false,
    first_name: "Kirill".to_string(),
    last_name: None,
    username: None
  };
  filter.process_url("https://www.youtube.com/watch?v=xuc9C-C6Ldw", Some(&user)).unwrap();
}

#[test]
fn regex_id_test() {
  let r = Regex::new(r"(v=|youtu.be/)(?P<id>[^&]*)").unwrap();
  // let s = "https://www.youtube.com/watch?v=xuc9C-C6Ldw";
  let s = "https://youtu.be/xuc9C-C6Ldw";
  let captures = r.captures(s).unwrap();
  let id = captures.name("id").unwrap().as_str().to_string();

  assert_eq!("xuc9C-C6Ldw", id);
}