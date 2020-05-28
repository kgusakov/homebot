use std::{
  path::Path,
  env,
  process::{Command, Output},
  io::Read,
};
use crate::{BResult, Filter, ProcessingResult, Message};

use std::fs::File;

use futures::prelude::*;

use rusoto_s3::{S3, S3Client, PutObjectRequest};

use regex::Regex;


struct PodcastFilter {
  youtube_extractor: String,
  id_regex: Regex,
  tmp_dir: String,
  bucket_name: String,
  s3_client: S3Client,
}

impl PodcastFilter {

  pub fn new() -> Self {
    let youtube_extractor = {
      env::var("YOUTUBE_EXTRACTOR")
          .expect("Provide YOUTUBE_EXTRACTOR environment variable please")
    };
    let tmp_dir = {
      env::var("BOT_TMP_DIR")
        .expect("Provide BOT_TMP_DIR environment variable please")
    };

    let bucket_name = {
      env::var("BOT_BUCKET_NAME")
        .expect("Provide BOT_BUCKET_NAME environment variable please")
    };
    Self { youtube_extractor,
          id_regex: Regex::new(r"v=(?P<id>[[:alnum:]]*)").expect("Can't compile video id Regex"),
          tmp_dir,
          bucket_name,
          s3_client: {
            let region = rusoto_core::region::Region::Custom {
              name: "ru-central1".to_owned(),
              endpoint: "storage.yandexcloud.net".to_owned(),
            };
        
            rusoto_s3::S3Client::new(region)
          }
    }
  }

  fn process_url(&self, url: &str) -> ProcessingResult {
    let video_id = self.extract_id(url)?;
        let download_path = Path::new(&self.tmp_dir).join("%(id)s.%(ext)s");
        self.download(url, &download_path)?;
        let downloaded_file_path = Path::new(&self.tmp_dir).join(format!("{}.mp3", video_id));
        self.upload_file(&downloaded_file_path, format!("data/{}.mp3", video_id))
  }

  fn download(&self, url: &str, path: &Path) -> BResult<Output> {
    Ok(Command::new(&self.youtube_extractor)
            .arg("-x")
            .args(&["--audio-format", "mp3"])
            .args(&["-o", path.to_str().ok_or("Can't stringify path for download file")?])
            .arg(url)
            .output()?)
  }

  fn extract_id(&self, s: &str) -> Result<String, Box<dyn std::error::Error>> {
    Ok(self.id_regex.captures(s).and_then(|cap| {
      cap.name("id").map(|id| id.as_str())}).ok_or("Can't parse video id from youtube url")?.to_owned())
  }

  fn upload_file(&self,file: &Path, s3_path: String) -> Result<(), Box<dyn std::error::Error>> {
    let mut body: Vec<u8> = vec![];
    File::open(file)?.read_to_end(&mut body)?;
    Ok(self.s3_client.put_object(PutObjectRequest {
      bucket: self.bucket_name.to_owned(),
      key: s3_path,
      body: Some(body.into()),
      ..Default::default()
    }).sync().map(|_| ())?)
  }
}

#[test]
fn test_download() {
  use std::io::{Write};
  env::set_var("YOUTUBE_EXTRACTOR", "youtube-dl");
  let filter = PodcastFilter::new();
  let result = filter 
    .download("https://www.youtube.com/watch?v=1BVPmUuZSlU", &Path::new("/tmp/%(id)s.%(ext)s"))
    .unwrap();
  std::io::stdout().write_all(&result.stdout).unwrap();
  std::io::stdout().write_all(&result.stderr).unwrap();
}

impl Filter for PodcastFilter {
  fn process(&self, m: &Message) -> ProcessingResult {
    match &m.text {
      Some(s) if s.starts_with("https://www.youtube.com/watch") => {
        self.process_url(s)
      },
      _ => Ok(())
    }
  }
}


struct StorageClient;

impl StorageClient {

  
}

#[test]
fn process_url_test() {
  let filter = PodcastFilter::new();
  filter.process_url("https://www.youtube.com/watch?v=1BVPmUuZSlU").unwrap();
}