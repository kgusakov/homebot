use super::s3_storage::S3Storage;
use anyhow::{Context, Result};
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

pub struct MetadataStorage {
    s3_storage: S3Storage,
}

impl MetadataStorage {
    pub fn new() -> Self {
        Self {
            s3_storage: S3Storage::new(),
        }
    }

    pub async fn load_metadata<'a>(&self, s3_path: &'a str) -> Result<VecDeque<VideoMetadata>> {
        Ok(match self.s3_storage.download_object(s3_path).await {
            Ok(d) => Ok(rmp_serde::from_read(d.as_slice())?),
            Err(e) => match e.downcast_ref::<rusoto_core::RusotoError<rusoto_s3::GetObjectError>>()
            {
                Some(rusoto_core::RusotoError::Service(rusoto_s3::GetObjectError::NoSuchKey(
                    _,
                ))) => Ok(VecDeque::new()),
                _ => Err(e)
                    .with_context(|| format!("Can't load metadata from the path: '{}'", s3_path)),
            },
        }?)
    }

    pub async fn update_metadata(
        &self,
        s3_path: &str,
        data: &VecDeque<VideoMetadata>,
    ) -> Result<()> {
        let mut buf = Vec::new();
        data.serialize(&mut Serializer::new(&mut buf))
            .with_context(|| format!("Can't serialize metadata for the path: '{}'", s3_path))?;
        self.s3_storage
            .upload_object(buf, s3_path)
            .await
            .with_context(|| format!("Can't upload metadata to the path: '{}'", s3_path))?;
        Ok(())
    }

    pub async fn add_metadata(
        &self,
        s3_path: &str,
        data: VideoMetadata,
    ) -> Result<VecDeque<VideoMetadata>> {
        let mut metadata = self.load_metadata(s3_path).await?;
        metadata.push_front(data);
        self.update_metadata(s3_path, &metadata).await?;
        Ok(metadata)
    }
}
