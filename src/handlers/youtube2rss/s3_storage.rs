use anyhow;
use rusoto_s3::{GetObjectRequest, PutObjectRequest, S3Client, S3};
use std::env;
use std::fs::File;
use std::io::Read;
use std::path::Path;

pub struct S3Storage {
    s3_client: S3Client,
    bucket_name: String,
}

impl S3Storage {
    pub fn new() -> Self {
        let bucket_name = {
            env::var("BOT_BUCKET_NAME")
                .expect("Provide BOT_BUCKET_NAME environment variable please")
        };
        Self {
            s3_client: {
                let region = rusoto_core::region::Region::Custom {
                    name: "ru-central1".to_owned(),
                    endpoint: "storage.yandexcloud.net".to_owned(),
                };

                rusoto_s3::S3Client::new(region)
            },
            bucket_name,
        }
    }

    pub fn download_object(&self, s3_path: &str) -> anyhow::Result<Vec<u8>> {
        let response = self
            .s3_client
            .get_object(GetObjectRequest {
                bucket: self.bucket_name.to_owned(),
                key: s3_path.to_string(),
                ..Default::default()
            })
            .sync()?;
        if let Some(stream) = response.body {
            let mut buf = Vec::new();
            stream.into_blocking_read().read_to_end(&mut buf)?;
            Ok(buf)
        } else {
            Ok(Vec::new())
        }
    }

    pub fn upload_object(
        &self,
        data: Vec<u8>,
        s3_path: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Ok(self
            .s3_client
            .put_object(PutObjectRequest {
                bucket: self.bucket_name.to_owned(),
                key: s3_path.to_string(),
                body: Some(data.into()),
                ..Default::default()
            })
            .sync()
            .map(|_| ())?)
    }

    pub fn upload_file(
        &self,
        file: &Path,
        s3_path: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut body: Vec<u8> = vec![];
        File::open(file)?.read_to_end(&mut body)?;
        Ok(self
            .s3_client
            .put_object(PutObjectRequest {
                bucket: self.bucket_name.to_owned(),
                key: s3_path.to_string(),
                body: Some(body.into()),
                ..Default::default()
            })
            .sync()
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
