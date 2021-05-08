/*!
# Download Response for Rocket Framework

This crate provides a response struct used for client downloading.

See `examples`.
*/

pub extern crate mime;
extern crate mime_guess;
extern crate rocket;
extern crate url_escape;
#[macro_use]
extern crate educe;

mod temp_file_async_reader;

use std::io::{self, Cursor};
use std::marker::Unpin;
use std::path::Path;
use std::sync::Arc;

use mime::Mime;

use rocket::data::TempFile;
use rocket::http::Status;
use rocket::request::Request;
use rocket::response::{self, Responder, Response};

use rocket::tokio::fs::File as AsyncFile;
use rocket::tokio::io::AsyncRead;

use temp_file_async_reader::TempFileAsyncReader;

#[derive(Educe)]
#[educe(Debug)]
enum DownloadResponseData<'o> {
    Slice(&'o [u8]),
    Vec(Vec<u8>),
    Reader {
        #[educe(Debug(ignore))]
        data: Box<dyn AsyncRead + Send + Unpin + 'o>,
        content_length: Option<u64>,
    },
    File(Arc<Path>, AsyncFile),
    TempFile(Box<TempFile<'o>>),
}

pub type DownloadResponse = DownloadResponsePro<'static>;

#[derive(Debug)]
pub struct DownloadResponsePro<'o> {
    file_name: Option<String>,
    content_type: Option<Mime>,
    data: DownloadResponseData<'o>,
}

impl<'o> DownloadResponsePro<'o> {
    /// Create a `DownloadResponse` instance from a `&'o [u8]`.
    pub fn from_slice<S: Into<String>>(
        data: &'o [u8],
        file_name: Option<S>,
        content_type: Option<Mime>,
    ) -> DownloadResponsePro<'o> {
        let file_name = file_name.map(|file_name| file_name.into());

        let data = DownloadResponseData::Slice(data);

        DownloadResponsePro {
            file_name,
            content_type,
            data,
        }
    }

    /// Create a `DownloadResponse` instance from a `Vec<u8>`.
    pub fn from_vec<S: Into<String>>(
        vec: Vec<u8>,
        file_name: Option<S>,
        content_type: Option<Mime>,
    ) -> DownloadResponsePro<'o> {
        let file_name = file_name.map(|file_name| file_name.into());

        let data = DownloadResponseData::Vec(vec);

        DownloadResponsePro {
            file_name,
            content_type,
            data,
        }
    }

    /// Create a `DownloadResponse` instance from a reader.
    pub fn from_reader<R: AsyncRead + Send + Unpin + 'o, S: Into<String>>(
        reader: R,
        file_name: Option<S>,
        content_type: Option<Mime>,
        content_length: Option<u64>,
    ) -> DownloadResponsePro<'o> {
        let file_name = file_name.map(|file_name| file_name.into());

        let data = DownloadResponseData::Reader {
            data: Box::new(reader),
            content_length,
        };

        DownloadResponsePro {
            file_name,
            content_type,
            data,
        }
    }

    /// Create a `DownloadResponse` instance from a path of a file.
    pub async fn from_file<P: Into<Arc<Path>>, S: Into<String>>(
        path: P,
        file_name: Option<S>,
        content_type: Option<Mime>,
    ) -> Result<DownloadResponsePro<'o>, io::Error> {
        let path = path.into();

        let file = AsyncFile::open(path.as_ref()).await?;

        let file_name = file_name.map(|file_name| file_name.into());

        let data = DownloadResponseData::File(path, file);

        Ok(DownloadResponsePro {
            file_name,
            content_type,
            data,
        })
    }

    /// Create a `DownloadResponse` instance from a `TempFile`.
    pub fn from_temp_file<S: Into<String>>(
        temp_file: TempFile<'o>,
        file_name: Option<S>,
        content_type: Option<Mime>,
    ) -> DownloadResponsePro<'o> {
        let file_name = file_name.map(|file_name| file_name.into());

        let data = DownloadResponseData::TempFile(Box::new(temp_file));

        DownloadResponsePro {
            file_name,
            content_type,
            data,
        }
    }
}

macro_rules! file_name {
    ($s:expr, $res:expr) => {
        if let Some(file_name) = $s.file_name {
            if file_name.is_empty() {
                $res.raw_header("Content-Disposition", "attachment");
            } else {
                let mut v = String::from("attachment; filename*=UTF-8''");

                url_escape::encode_component_to_string(file_name, &mut v);

                $res.raw_header("Content-Disposition", v);
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

impl<'r, 'o: 'r> Responder<'r, 'o> for DownloadResponsePro<'o> {
    fn respond_to(self, _: &'r Request<'_>) -> response::Result<'o> {
        let mut response = Response::build();

        match self.data {
            DownloadResponseData::Slice(data) => {
                file_name!(self, response);
                content_type!(self, response);

                response.sized_body(data.len(), Cursor::new(data));
            }
            DownloadResponseData::Vec(data) => {
                file_name!(self, response);
                content_type!(self, response);

                response.sized_body(data.len(), Cursor::new(data));
            }
            DownloadResponseData::Reader {
                data,
                content_length,
            } => {
                file_name!(self, response);
                content_type!(self, response);

                if let Some(content_length) = content_length {
                    response.raw_header("Content-Length", content_length.to_string());
                }

                response.streamed_body(data);
            }
            DownloadResponseData::File(path, file) => {
                if let Some(file_name) = self.file_name {
                    if file_name.is_empty() {
                        response.raw_header("Content-Disposition", "attachment");
                    } else {
                        let mut v = String::from("attachment; filename*=UTF-8''");

                        url_escape::encode_component_to_string(file_name, &mut v);

                        response.raw_header("Content-Disposition", v);
                    }
                } else if let Some(file_name) =
                    path.file_name().map(|file_name| file_name.to_string_lossy())
                {
                    let mut v = String::from("attachment; filename*=UTF-8''");

                    url_escape::encode_component_to_string(file_name, &mut v);

                    response.raw_header("Content-Disposition", v);
                } else {
                    response.raw_header("Content-Disposition", "attachment");
                }

                if let Some(content_type) = self.content_type {
                    response.raw_header("Content-Type", content_type.to_string());
                } else if let Some(extension) = path.extension() {
                    if let Some(extension) = extension.to_str() {
                        let content_type = mime_guess::from_ext(extension).first_or_octet_stream();

                        response.raw_header("Content-Type", content_type.to_string());
                    }
                }

                response.sized_body(None, file);
            }
            DownloadResponseData::TempFile(file) => {
                if let Some(file_name) = self.file_name {
                    if file_name.is_empty() {
                        response.raw_header("Content-Disposition", "attachment");
                    } else {
                        let mut v = String::from("attachment; filename*=UTF-8''");

                        url_escape::encode_component_to_string(file_name, &mut v);

                        response.raw_header("Content-Disposition", v);
                    }
                } else if let Some(file_name) = file.name() {
                    if file_name.is_empty() {
                        response.raw_header("Content-Disposition", "attachment");
                    } else {
                        let mut v = String::from("attachment; filename*=UTF-8''");

                        url_escape::encode_component_to_string(file_name, &mut v);

                        response.raw_header("Content-Disposition", v);
                    }
                } else {
                    response.raw_header("Content-Disposition", "attachment");
                }

                if let Some(content_type) = self.content_type {
                    response.raw_header("Content-Type", content_type.to_string());
                } else if let Some(content_type) = file.content_type() {
                    response.raw_header("Content-Type", content_type.to_string());
                } else if let Some(extension) = file.name().map(Path::new).and_then(Path::extension)
                {
                    if let Some(extension) = extension.to_str() {
                        let content_type = mime_guess::from_ext(extension).first_or_octet_stream();

                        response.raw_header("Content-Type", content_type.to_string());
                    }
                }

                response.raw_header("Content-Length", file.len().to_string());

                response.streamed_body(
                    TempFileAsyncReader::from(file).map_err(|_| Status::InternalServerError)?,
                );
            }
        }

        response.ok()
    }
}
