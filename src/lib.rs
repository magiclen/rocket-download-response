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

use std::fs::File;
use std::io::{Cursor, ErrorKind};
use std::marker::Unpin;
use std::path::Path;
use std::rc::Rc;

use mime::Mime;

use rocket::http::Status;
use rocket::request::Request;
use rocket::response::{self, Responder, Response};

use rocket::tokio::fs::File as AsyncFile;
use rocket::tokio::io::AsyncRead;

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
    File(Rc<Path>),
}

pub type DownloadResponse = DownloadResponsePro<'static>;

#[derive(Debug)]
pub struct DownloadResponsePro<'o> {
    file_name: Option<String>,
    content_type: Option<Mime>,
    data: DownloadResponseData<'o>,
}

impl<'o> DownloadResponsePro<'o> {
    /// Create a `DownloadResponse` instance from a `&'r [u8]`.
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
    pub fn from_file<P: Into<Rc<Path>>, S: Into<String>>(
        path: P,
        file_name: Option<S>,
        content_type: Option<Mime>,
    ) -> DownloadResponsePro<'o> {
        let path = path.into();
        let file_name = file_name.map(|file_name| file_name.into());

        let data = DownloadResponseData::File(path);

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
            DownloadResponseData::File(path) => {
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

                let file = File::open(path).map_err(|err| {
                    if err.kind() == ErrorKind::NotFound {
                        Status::NotFound
                    } else {
                        Status::InternalServerError
                    }
                })?;

                response.sized_body(None, AsyncFile::from_std(file));
            }
        }

        response.ok()
    }
}
