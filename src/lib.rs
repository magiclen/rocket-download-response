/*!
# Download Response for Rocket Framework

This crate provides a response struct used for client downloading.

```rust
use rocket::get;
use rocket_download_response::DownloadResponse;

#[get("/")]
fn index() -> DownloadResponse {
    DownloadResponse::from_vec(b"Hello world!".to_vec()).file_name("hello.txt")
}
```

The media type is guessed from the extension of the file name, but it can also be set explicitly.

```rust
use rocket::get;
use rocket_download_response::{DownloadResponse, mime};

#[get("/")]
fn index() -> DownloadResponse {
    DownloadResponse::from_vec(b"a,b\n1,2\n".to_vec())
        .file_name("report.csv")
        .content_type(mime::TEXT_CSV)
}
```

A file on disk is opened asynchronously, and both the file name and the media type default to what the path says.

```rust
use std::io;

use rocket::get;
use rocket_download_response::DownloadResponse;

#[get("/")]
async fn index() -> io::Result<DownloadResponse> {
    DownloadResponse::from_file("images/image.jpg").await
}
```

A reader is sent as it is.
Its length can be given so that a `Content-Length` header is sent along with it.

```rust
use std::io::Cursor;

use rocket::get;
use rocket_download_response::DownloadResponse;

#[get("/")]
fn index() -> DownloadResponse {
    DownloadResponse::from_reader_sized(Cursor::new(b"Hello world!"), 12)
        .file_name("hello.txt")
}
```

An uploaded file can be sent straight back.
Rocket drops the extension from `TempFile::name`, so this crate uses the original file name to recover it and falls back to the upload's media type.

```rust
use std::io;

use rocket::{form::Form, fs::TempFile, post};
use rocket_download_response::DownloadResponsePro;

#[post("/", data = "<file>")]
async fn echo(file: Form<TempFile<'_>>) -> io::Result<DownloadResponsePro<'_>> {
    DownloadResponsePro::from_temp_file(file.into_inner()).await
}
```

`inline` asks the client to display the body instead of saving it.

```rust
use rocket::get;
use rocket_download_response::DownloadResponse;

#[get("/")]
fn index() -> DownloadResponse {
    DownloadResponse::from_vec(b"Hello world!".to_vec())
        .file_name("hello.txt")
        .inline()
}
```
*/

pub extern crate mime;
pub extern crate rocket;

mod temp_file_async_reader;

use std::{
    borrow::Cow,
    ffi::OsStr,
    io::{self, Cursor},
    path::Path,
};

use educe::Educe;
use mime::Mime;
use rocket::{
    fs::{FileName, TempFile},
    request::Request,
    response::{self, Builder, Responder, Response},
    tokio::{fs::File as AsyncFile, io::AsyncRead},
};
use temp_file_async_reader::TempFileAsyncReader;

#[derive(Educe)]
#[educe(Debug)]
enum DownloadResponseData<'o> {
    Bytes(Cow<'o, [u8]>),
    Reader {
        #[educe(Debug(ignore))]
        data:           Box<dyn AsyncRead + Send + Unpin + 'o>,
        content_length: Option<u64>,
    },
    File {
        path: Box<Path>,
        file: AsyncFile,
        /// Read from the metadata of the file while it was opened, so the length of the body does not have to be found again by seeking.
        len:  u64,
    },
    TempFile {
        temp_file:  Box<TempFile<'o>>,
        async_file: AsyncFile,
        /// The length of the temporary file when it was opened.
        len:        u64,
    },
}

/// A [`DownloadResponsePro`] that borrows nothing, which is what a route usually returns.
pub type DownloadResponse = DownloadResponsePro<'static>;

/// The responder that makes a client save the body as a file.
#[derive(Debug)]
pub struct DownloadResponsePro<'o> {
    data:         DownloadResponseData<'o>,
    file_name:    Option<String>,
    content_type: Option<Mime>,
    inline:       bool,
}

impl<'o> DownloadResponsePro<'o> {
    #[inline]
    fn new(data: DownloadResponseData<'o>) -> Self {
        DownloadResponsePro {
            data,
            file_name: None,
            content_type: None,
            inline: false,
        }
    }

    /// Create a `DownloadResponse` instance from a `&'o [u8]`.
    #[inline]
    pub fn from_slice(data: &'o [u8]) -> Self {
        Self::new(DownloadResponseData::Bytes(Cow::Borrowed(data)))
    }

    /// Create a `DownloadResponse` instance from a `Vec<u8>`.
    #[inline]
    pub fn from_vec(vec: Vec<u8>) -> Self {
        Self::new(DownloadResponseData::Bytes(Cow::Owned(vec)))
    }

    /// Create a `DownloadResponse` instance from a reader whose length is not known in advance, so no `Content-Length` header is sent.
    #[inline]
    pub fn from_reader<R: AsyncRead + Send + Unpin + 'o>(reader: R) -> Self {
        Self::new(DownloadResponseData::Reader {
            data:           Box::new(reader),
            content_length: None,
        })
    }

    /// Create a `DownloadResponse` instance from a reader whose length is already known, so a `Content-Length` header can be sent.
    #[inline]
    pub fn from_reader_sized<R: AsyncRead + Send + Unpin + 'o>(
        reader: R,
        content_length: u64,
    ) -> Self {
        Self::new(DownloadResponseData::Reader {
            data:           Box::new(reader),
            content_length: Some(content_length),
        })
    }

    /// Create a `DownloadResponse` instance from a path of a file, which is opened right away.
    pub async fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, io::Error> {
        let path = path.as_ref();

        let file = AsyncFile::open(path).await?;
        let metadata = file.metadata().await?;

        Ok(Self::new(DownloadResponseData::File {
            path: Box::from(path),
            file,
            len: metadata.len(),
        }))
    }

    /// Create a `DownloadResponse` instance from a `TempFile`, which is opened right away when it is stored on disk.
    pub async fn from_temp_file(temp_file: TempFile<'o>) -> Result<Self, io::Error> {
        let (async_file, len) = match &temp_file {
            TempFile::File {
                path, ..
            } => {
                let async_file = AsyncFile::open(path).await?;
                let len = async_file.metadata().await?.len();

                (async_file, len)
            },
            // A `TempFile` buffered in memory has no name, no media type and no file on disk, so nothing but its content is worth keeping.
            TempFile::Buffered {
                content,
            } => return Ok(Self::new(DownloadResponseData::Bytes(Cow::Borrowed(content)))),
        };

        Ok(Self::new(DownloadResponseData::TempFile {
            temp_file: Box::new(temp_file),
            async_file,
            len,
        }))
    }

    /// Set the file name the client should save the body as.
    /// An empty name sends no name at all.
    #[inline]
    #[must_use]
    pub fn file_name<S: Into<String>>(mut self, file_name: S) -> Self {
        self.file_name = Some(file_name.into());

        self
    }

    /// Set the media type of the body, which otherwise is guessed from the file name.
    #[inline]
    #[must_use]
    pub fn content_type(mut self, content_type: Mime) -> Self {
        self.content_type = Some(content_type);

        self
    }

    /// Send `inline` instead of `attachment`, which asks the client to display the body rather than to save it.
    #[inline]
    #[must_use]
    pub fn inline(mut self) -> Self {
        self.inline = true;

        self
    }
}

const OCTET_STREAM: &str = "application/octet-stream";

const HEX_DIGITS: [u8; 16] = *b"0123456789ABCDEF";

/// Percent-encode `s` with the `attr-char` set of RFC 8187, which is the only set the `filename*` parameter accepts.
fn encode_attr_char_to_string(s: &str, output: &mut String) {
    for b in s.bytes() {
        if b.is_ascii_alphanumeric()
            || matches!(
                b,
                b'!' | b'#' | b'$' | b'&' | b'+' | b'-' | b'.' | b'^' | b'_' | b'`' | b'|' | b'~'
            )
        {
            output.push(b as char);
        } else {
            output.push('%');
            output.push(HEX_DIGITS[usize::from(b >> 4)] as char);
            output.push(HEX_DIGITS[usize::from(b & 0x0F)] as char);
        }
    }
}

/// Write `s` as the quoted-string of the `filename` parameter, replacing everything a quoted-string cannot hold with `_`.
fn encode_ascii_fallback_to_string(s: &str, output: &mut String) {
    for c in s.chars() {
        if c == ' ' || (c.is_ascii_graphic() && c != '"' && c != '\\') {
            output.push(c);
        } else {
            output.push('_');
        }
    }
}

const FILE_NAME_PREFIX: &str = "; filename=\"";
const FILE_NAME_STAR_PREFIX: &str = "\"; filename*=UTF-8''";

/// Write the `Content-Disposition` header, carrying the file name both as an ASCII fallback and as RFC 8187 UTF-8.
fn write_content_disposition(response: &mut Builder<'_>, inline: bool, file_name: Option<&str>) {
    let disposition = if inline { "inline" } else { "attachment" };

    let Some(file_name) = file_name.filter(|file_name| !file_name.is_empty()) else {
        // The disposition on its own is a `&'static str`, so no string has to be built for it.
        response.raw_header("Content-Disposition", disposition);

        return;
    };

    // The ASCII fallback keeps the length of the name, the RFC 8187 copy triples it at worst.
    let mut v = String::with_capacity(
        disposition.len()
            + FILE_NAME_PREFIX.len()
            + FILE_NAME_STAR_PREFIX.len()
            + file_name.len() * 4,
    );

    v.push_str(disposition);
    v.push_str(FILE_NAME_PREFIX);
    encode_ascii_fallback_to_string(file_name, &mut v);
    v.push_str(FILE_NAME_STAR_PREFIX);
    encode_attr_char_to_string(file_name, &mut v);

    response.raw_header("Content-Disposition", v);
}

/// Write the `Content-Type` header, preferring the explicit media type, then the one the source carries, then a guess made from the file name or from the path of the source.
fn write_content_type(
    response: &mut Builder<'_>,
    content_type: Option<Mime>,
    data: &DownloadResponseData<'_>,
    file_name: Option<&str>,
) {
    if let Some(content_type) = content_type {
        response.raw_header("Content-Type", content_type.to_string());

        return;
    }

    if let DownloadResponseData::TempFile {
        temp_file, ..
    } = data
        && let Some(content_type) = temp_file.content_type()
        // `application/octet-stream` is what a client sends when it does not know either, so a guess does better.
        && !content_type.is_binary()
    {
        response.raw_header("Content-Type", content_type.to_string());

        return;
    }

    let guess = |extension| mime_guess::from_ext(extension).first_raw();
    let extension = file_name.map(Path::new).and_then(Path::extension).and_then(OsStr::to_str);
    if let Some(content_type) = extension.and_then(guess) {
        response.raw_header("Content-Type", content_type);

        return;
    }

    let source_extension = match data {
        DownloadResponseData::File {
            path, ..
        } => path.extension().and_then(OsStr::to_str),
        _ => None,
    };

    response.raw_header("Content-Type", source_extension.and_then(guess).unwrap_or(OCTET_STREAM));
}

/// Recover the file name and its extensions from the upload.
fn temp_file_name<'a>(temp_file: &'a TempFile<'_>) -> Option<Cow<'a, str>> {
    let raw_name = temp_file.raw_name()?.dangerous_unsafe_unsanitized_raw().as_str();
    let file_name = raw_name.rsplit(['/', '\\']).next()?;
    let name = FileName::new(file_name).as_str()?;

    if let Some(suffix) = file_name.strip_prefix(name).and_then(|suffix| suffix.strip_prefix('.'))
        && suffix.split('.').all(|part| FileName::new(part).is_safe())
    {
        return Some(Cow::Borrowed(file_name));
    }

    if let Some(extension) = Path::new(file_name).extension().and_then(OsStr::to_str)
        && FileName::new(extension).is_safe()
    {
        return Some(Cow::Owned(format!("{name}.{extension}")));
    }

    match temp_file.content_type().and_then(|content_type| content_type.extension()) {
        Some(extension) => Some(Cow::Owned(format!("{name}.{extension}"))),
        None => Some(Cow::Borrowed(name)),
    }
}

/// The file name the source suggests, used when the caller sets none.
fn default_file_name<'a>(data: &'a DownloadResponseData<'_>) -> Option<Cow<'a, str>> {
    match data {
        DownloadResponseData::File {
            path, ..
        } => path.file_name().map(OsStr::to_string_lossy),
        DownloadResponseData::TempFile {
            temp_file, ..
        } => temp_file_name(temp_file),
        _ => None,
    }
}

impl<'r, 'o: 'r> Responder<'r, 'o> for DownloadResponsePro<'o> {
    fn respond_to(self, _: &'r Request<'_>) -> response::Result<'o> {
        let DownloadResponsePro {
            data,
            file_name,
            content_type,
            inline,
        } = self;

        let mut response = Response::build();

        {
            let file_name = file_name.map(Cow::Owned).or_else(|| default_file_name(&data));

            write_content_disposition(&mut response, inline, file_name.as_deref());
            write_content_type(&mut response, content_type, &data, file_name.as_deref());
        }

        match data {
            DownloadResponseData::Bytes(data) => {
                response.sized_body(data.len(), Cursor::new(data));
            },
            DownloadResponseData::Reader {
                data,
                content_length,
            } => {
                if let Some(content_length) = content_length {
                    response.raw_header("Content-Length", content_length.to_string());
                }

                response.streamed_body(data);
            },
            DownloadResponseData::File {
                file,
                len,
                ..
            } => {
                if let Ok(len) = usize::try_from(len) {
                    response.sized_body(len, file);
                } else {
                    // Keep the full length on 32-bit targets.
                    response.raw_header("Content-Length", len.to_string()).streamed_body(file);
                }
            },
            DownloadResponseData::TempFile {
                temp_file,
                async_file,
                len,
            } => {
                response.raw_header("Content-Length", len.to_string());

                response.streamed_body(TempFileAsyncReader::new(temp_file, async_file));
            },
        }

        response.ok()
    }
}
