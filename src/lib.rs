/*!
# Download Response for Rocket Framework

This crate provides a response struct used for client downloading.

See `examples`.
*/

pub extern crate mime;
extern crate mime_guess;
extern crate percent_encoding;
extern crate rocket;
#[macro_use]
extern crate derivative;

use std::io::{Read, Cursor};
use std::fs::File;
use std::path::Path;
use std::rc::Rc;

use mime::Mime;

use rocket::response::{self, Response, Responder};
use rocket::request::Request;
use rocket::http::Status;

#[derive(Derivative)]
#[derivative(Debug)]
enum DownloadResponseData {
    Vec(Vec<u8>),
    Reader {
        #[derivative(Debug = "ignore")]
        data: Box<Read + 'static>,
        content_length: Option<u64>,
    },
    File(Rc<Path>),
}

#[derive(Debug)]
pub struct DownloadResponse {
    file_name: Option<String>,
    content_type: Option<Mime>,
    data: DownloadResponseData,
}

impl DownloadResponse {
    /// Create a `DownloadResponse` instance from a `Vec<u8>`.
    pub fn from_vec<S: Into<String>>(vec: Vec<u8>, file_name: Option<S>, content_type: Option<Mime>) -> DownloadResponse {
        let file_name = file_name.map(|file_name| file_name.into());

        let data = DownloadResponseData::Vec(vec);

        DownloadResponse {
            file_name,
            content_type,
            data,
        }
    }

    /// Create a `DownloadResponse` instance from a reader.
    pub fn from_reader<R: Read + 'static, S: Into<String>>(reader: R, file_name: Option<S>, content_type: Option<Mime>, content_length: Option<u64>) -> DownloadResponse {
        let file_name = file_name.map(|file_name| file_name.into());

        let data = DownloadResponseData::Reader {
            data: Box::new(reader),
            content_length,
        };

        DownloadResponse {
            file_name,
            content_type,
            data,
        }
    }

    /// Create a `DownloadResponse` instance from a path of a file.
    pub fn from_file<P: Into<Rc<Path>>, S: Into<String>>(path: P, file_name: Option<S>, content_type: Option<Mime>) -> DownloadResponse {
        let path = path.into();
        let file_name = file_name.map(|file_name| file_name.into());

        let data = DownloadResponseData::File(path);

        DownloadResponse {
            file_name,
            content_type,
            data,
        }
    }
}

macro_rules! file_name {
    ($s:expr, $res:expr) => {
        if let Some(file_name) = $s.file_name {
            if !file_name.is_empty() {
                $res.raw_header("Content-Disposition", format!("inline; filename*=UTF-8''{}", percent_encoding::percent_encode(file_name.as_bytes(), percent_encoding::QUERY_ENCODE_SET)));
            }
        }
    };
}

macro_rules! content_type {
    ($s:expr, $res:expr) => {
        if let Some(content_type) = $s.content_type {
            $res.raw_header("Content-Type", content_type.to_string());
        }
    };
}

impl<'a> Responder<'a> for DownloadResponse {
    fn respond_to(self, _: &Request) -> response::Result<'a> {
        let mut response = Response::build();

        match self.data {
            DownloadResponseData::Vec(data) => {
                file_name!(self, response);
                content_type!(self, response);

                response.sized_body(Cursor::new(data));
            }
            DownloadResponseData::Reader { data, content_length } => {
                file_name!(self, response);
                content_type!(self, response);

                if let Some(content_length) = content_length {
                    response.raw_header("Content-Length", content_length.to_string());
                }

                response.streamed_body(data);
            }
            DownloadResponseData::File(path) => {
                if let Some(file_name) = self.file_name {
                    if !file_name.is_empty() {
                        response.raw_header("Content-Disposition", format!("inline; filename*=UTF-8''{}", percent_encoding::percent_encode(file_name.as_bytes(), percent_encoding::QUERY_ENCODE_SET)));
                    }
                } else {
                    if let Some(file_name) = path.file_name().map(|file_name| file_name.to_string_lossy()) {
                        response.raw_header("Content-Disposition", format!("inline; filename*=UTF-8''{}", percent_encoding::percent_encode(file_name.as_bytes(), percent_encoding::QUERY_ENCODE_SET)));
                    }
                }
                content_type!(self, response);

                let metadata = path.metadata().map_err(|_| Status::InternalServerError)?;

                response.raw_header("Content-Length", metadata.len().to_string());

                response.streamed_body(File::open(path).map_err(|_| Status::InternalServerError)?);
            }
        }

        response.ok()
    }
}