use std::time::SystemTime;
use serde::{Serialize, Deserialize};
use rmp_serde;
use rmp_serde::Serializer;
use super::s3_storage::S3Storage;
use anyhow;


#[derive(Debug, Serialize, Deserialize)]
pub struct VideoMetadata {
  pub file_size: u64,
  pub file_url: String,
  pub video_id: String,
  pub created_at: SystemTime,
  pub name: String,
  pub original_link: String,
}

pub struct  Metadata {
  s3_storage: S3Storage
}

impl Metadata {

  pub fn new() -> Self {
    Self {
      s3_storage: S3Storage::new()
    }
  }

  pub fn load_metadata<'a>(&self, s3_path: &'a str) -> Result<Vec<VideoMetadata>, Box<dyn std::error::Error>>  {
    Ok(match self.s3_storage.download_object(s3_path) {
      Ok(d) => Ok(rmp_serde::from_read(d.as_slice())?),
      Err(e) => 
        match e.downcast_ref::<rusoto_core::RusotoError<rusoto_s3::GetObjectError>>() {
          Some(rusoto_core::RusotoError::Service(rusoto_s3::GetObjectError::NoSuchKey(_))) => Ok(Vec::new()),
          _ => Err(e)
      },
    }?)
  }

  pub fn update_metadata(&self, s3_path: &str, data: Vec<VideoMetadata>) -> Result<(), Box<dyn std::error::Error>> {
    let mut buf = Vec::new();
    data.serialize(&mut Serializer::new(&mut buf))?;
    self.s3_storage.upload_object(buf, s3_path)
  }
}


// #[test]
// fn bechmark_test() {
//   use serde_json::json;
//   use rmp_serde::{Deserializer, Serializer};
//   // 5 per day, 
//   let limit = 5*365*10;
//   let mut storage = vec![];
//   for _ in 1..limit { 
//     let data = VideoMetadata {
//       video_id: "uREnLAXv_y0".to_string(),
//       created_at: SystemTime::now(),
//       name: "5 отличий хорошего гитарного мастера от плохого (бомбление)".to_string(),
//     };
//     storage.push(data);
//   }
//   use std::time::Instant;
//   let start = Instant::now();
//   let json = serde_json::to_string(&storage).unwrap();
//   println!("JSON serialize {:?}ms {} bytes", start.elapsed().as_millis(), json.len());

//   let start = Instant::now();
//   let data: Vec<VideoMetadata> = serde_json::from_str(&json).unwrap();
//   println!("JSON deserialize {:?}ms {} rows", start.elapsed().as_millis(), data.len());

//   let start = Instant::now();
//   let mut buf = Vec::new();
//   storage.serialize(&mut Serializer::new(&mut buf)).unwrap();
//   println!("MessagePack serialize {:?}ms {} bytes", start.elapsed().as_millis(), buf.len());

//   let start = Instant::now();
//   let data: Vec<VideoMetadata> = rmp_serde::from_read_ref(&buf).unwrap();
//   println!("MessagePack deserialize {:?}ms {} rows", start.elapsed().as_millis(), data.len());
// }
