use std::{borrow::Cow, io, path::Path};

use fs_err as fs;
use reqwest::StatusCode;
use thiserror::Error;

use crate::roblox_web_api::{ImageUploadData, RobloxApiClient, RobloxApiError};

pub trait SyncBackend {
    fn upload(&mut self, data: UploadInfo) -> Result<UploadResponse, Error>;
}

pub struct UploadResponse {
    pub id: u64,
}

pub struct UploadInfo {
    pub name: String,
    pub contents: Vec<u8>,
    pub hash: String,
}

pub struct RobloxSyncBackend<'a> {
    api_client: &'a mut RobloxApiClient,
    upload_to_group_id: Option<u64>,
}

impl<'a> RobloxSyncBackend<'a> {
    pub fn new(api_client: &'a mut RobloxApiClient, upload_to_group_id: Option<u64>) -> Self {
        Self {
            api_client,
            upload_to_group_id,
        }
    }
}

impl<'a> SyncBackend for RobloxSyncBackend<'a> {
    fn upload(&mut self, data: UploadInfo) -> Result<UploadResponse, Error> {
        log::info!("Uploading {} to Roblox", &data.name);

        let result = self
            .api_client
            .upload_image_with_moderation_retry(ImageUploadData {
                image_data: Cow::Owned(data.contents),
                name: &data.name,
                description: "Uploaded by Tarmac.",
                group_id: self.upload_to_group_id,
            });

        match result {
            Ok(response) => {
                log::info!(
                    "Uploaded {} to ID {}",
                    &data.name,
                    response.backing_asset_id
                );

                Ok(UploadResponse {
                    id: response.backing_asset_id,
                })
            }

            Err(RobloxApiError::ResponseError {
                status: StatusCode::TOO_MANY_REQUESTS,
                ..
            }) => Err(Error::RateLimited),

            Err(err) => Err(err.into()),
        }
    }
}

pub struct NoneSyncBackend;

impl SyncBackend for NoneSyncBackend {
    fn upload(&mut self, _data: UploadInfo) -> Result<UploadResponse, Error> {
        Err(Error::NoneBackend)
    }
}

pub struct DebugSyncBackend {
    last_id: u64,
}

impl DebugSyncBackend {
    pub fn new() -> Self {
        Self { last_id: 0 }
    }
}

impl SyncBackend for DebugSyncBackend {
    fn upload(&mut self, data: UploadInfo) -> Result<UploadResponse, Error> {
        log::info!("Copying {} to local folder", &data.name);

        self.last_id += 1;
        let id = self.last_id;

        let path = Path::new(".tarmac-debug");
        fs::create_dir_all(path)?;

        let file_path = path.join(id.to_string());
        fs::write(&file_path, &data.contents)?;

        Ok(UploadResponse { id })
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("Cannot upload assets with the 'none' target.")]
    NoneBackend,

    #[error("Tarmac was rate-limited trying to upload assets. Try again in a little bit.")]
    RateLimited,

    #[error(transparent)]
    Io {
        #[from]
        source: io::Error,
    },

    #[error(transparent)]
    RobloxError {
        #[from]
        source: RobloxApiError,
    },
}
