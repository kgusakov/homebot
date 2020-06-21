use super::s3_storage::S3Storage;
use rmp_serde;
use rmp_serde::Serializer;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::SystemTime;

#[derive(Debug, Serialize, Deserialize)]
pub struct VideoMetadata {
    pub file_size: u64,
    pub file_url: String,
    pub video_id: String,
    pub created_at: SystemTime,
    pub name: String,
    pub original_link: String,
}

pub struct Metadata {
    s3_storage: S3Storage,
}

impl Metadata {
    pub fn new() -> Self {
        Self {
            s3_storage: S3Storage::new(),
        }
    }

    pub fn load_metadata<'a>(
        &self,
        s3_path: &'a str,
    ) -> Result<VecDeque<VideoMetadata>, Box<dyn std::error::Error>> {
        Ok(match self.s3_storage.download_object(s3_path) {
            Ok(d) => Ok(rmp_serde::from_read(d.as_slice())?),
            Err(e) => match e.downcast_ref::<rusoto_core::RusotoError<rusoto_s3::GetObjectError>>()
            {
                Some(rusoto_core::RusotoError::Service(rusoto_s3::GetObjectError::NoSuchKey(
                    _,
                ))) => Ok(VecDeque::new()),
                _ => Err(e),
            },
        }?)
    }

    pub fn update_metadata(
        &self,
        s3_path: &str,
        data: &VecDeque<VideoMetadata>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut buf = Vec::new();
        data.serialize(&mut Serializer::new(&mut buf))?;
        self.s3_storage.upload_object(buf, s3_path)
    }
}
