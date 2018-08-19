//! # Download Response for Rocket Framework
//! This crate provides a response struct used for client downloading.

extern crate mime_guess;
extern crate rocket;

use std::io::{self, Read, ErrorKind};
use std::fs::{self, File};
use std::path::Path;

use mime_guess::get_mime_type_str;

use rocket::response::{self, Response, Responder};
use rocket::request::Request;

#[doc(hidden)]
pub const DOWNLOAD_RESPONSE_CHUNK_SIZE: u64 = 4096;

/// The response struct used for client downloading.
pub struct DownloadResponse {
    pub data: Box<Read>,
    pub attach_name: String,
    pub content_type: Option<String>,
    pub content_length: Option<u64>,
}

impl<'a> Responder<'a> for DownloadResponse {
    fn respond_to(self, _: &Request) -> response::Result<'a> {
        let mut response = Response::build();

        response
            .raw_header("Content-Disposition", format!("attachment; filename={}", self.attach_name))
            .raw_header("Content-Transfer-Encoding", "binary");

        if let Some(content_type) = self.content_type {
            response.raw_header("Content-Type", content_type);
        }

        if let Some(content_length) = self.content_length {
            response.raw_header("Content-Length", content_length.to_string());
        }

        response.chunked_body(self.data, DOWNLOAD_RESPONSE_CHUNK_SIZE);

        response.ok()
    }
}

impl DownloadResponse {
    /// Create a DownloadResponse instance from a path of a file.
    pub fn from_file<P: AsRef<Path>>(path: P) -> io::Result<DownloadResponse> {
        let path = path.as_ref();

        if !path.exists() {
            return Err(io::Error::from(ErrorKind::NotFound));
        }

        if !path.is_file() {
            return Err(io::Error::from(ErrorKind::InvalidInput));
        }

        let file_name = path.file_name().unwrap().to_str().unwrap().to_string();

        let file_size = match fs::metadata(&path) {
            Ok(metadata) => {
                Some(metadata.len())
            }
            Err(e) => return Err(e)
        };

        let content_type = match path.extension() {
            Some(extension) => {
                get_mime_type_str(&extension.to_str().unwrap().to_lowercase()).map(|t| { String::from(t) })
            }
            None => None
        };

        let data = Box::from(File::open(&path)?);

        Ok(DownloadResponse {
            data,
            attach_name: file_name,
            content_type,
            content_length: file_size,
        })
    }
}