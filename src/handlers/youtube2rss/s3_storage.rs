use anyhow::{Context, Result};
use rusoto_s3::{GetObjectRequest, PutObjectRequest, S3Client, S3};
use std::env;
use std::path::PathBuf;
use tokio::prelude::*;
use std::io::Read;

pub struct S3Storage {
    bucket_name: String,
}

impl S3Storage {
    fn s3_client(&self) -> S3Client {
        let region = rusoto_core::region::Region::Custom {
            name: "ru-central1".to_owned(),
            endpoint: "storage.yandexcloud.net".to_owned(),
        };

        rusoto_s3::S3Client::new(region)
    }

    pub fn new() -> Self {
        let bucket_name = {
            env::var("BOT_BUCKET_NAME")
                .expect("Provide BOT_BUCKET_NAME environment variable please")
        };
        Self { bucket_name }
    }

    pub async fn download_object(&self, s3_path: &str) -> Result<Vec<u8>> {
        let response = self
            .s3_client()
            .get_object(GetObjectRequest {
                bucket: self.bucket_name.to_owned(),
                key: s3_path.to_string(),
                ..Default::default()
            })
            .await
            .with_context(|| {
                format!(
                    "Can't GetObject with the path '{}' for downloading",
                    s3_path
                )
            })?;
        if let Some(stream) = response.body {
            let mut buf = Vec::new();
            stream
                .into_async_read()
                .read_to_end(&mut buf)
                .await
                .with_context(|| {
                    format!(
                        "Failed to read response body for downloading the object {}",
                        s3_path
                    )
                })?;
            Ok(buf)
        } else {
            Ok(Vec::new())
        }
    }

    pub async fn upload_object(&self, data: Vec<u8>, s3_path: &str) -> anyhow::Result<()> {
        Ok(self
            .s3_client()
            .put_object(PutObjectRequest {
                bucket: self.bucket_name.to_owned(),
                key: s3_path.to_string(),
                body: Some(data.into()),
                ..Default::default()
            })
            .await
            .with_context(|| format!("Failed to upload the object {}", s3_path))
            .map(|_| ())?)
    }

    pub async fn upload_file(&self, file: PathBuf, s3_path: String) -> Result<()> {
        let body = {
            let f_p = file.clone();
            let s3_p = s3_path.clone();
            tokio::task::spawn_blocking(move || {
                let mut f = std::fs::File::open(f_p)
                    .with_context(|| {
                        format!(
                            "Failed to open file during file upload to the path {}",
                            s3_p
                        )
                    })?;
                let mut body: Vec<u8> = vec![];
                f.read_to_end(&mut body)?;
                Ok::<Vec<u8>, anyhow::Error>(body)
                
            }).await?
        }?;

        Ok(self
            .s3_client()
            .put_object(PutObjectRequest {
                bucket: self.bucket_name.to_owned(),
                key: s3_path.to_string(),
                body: Some(body.into()),
                ..Default::default()
            })
            .await
            .with_context(|| format!("Failed to put object {}", s3_path))
            .map(|_| ())?)
    }

    pub fn get_public_url(&self, s3_path: &str) -> String {
        format!(
            "https://storage.yandexcloud.net/{bucket}/{file}",
            bucket = self.bucket_name,
            file = s3_path
        )
    }
}
