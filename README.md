Download Response for Rocket Framework
====================

[![CI](https://github.com/magiclen/rocket-download-response/actions/workflows/ci.yml/badge.svg)](https://github.com/magiclen/rocket-download-response/actions/workflows/ci.yml)

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

An uploaded file can be sent straight back. Rocket drops the extension from `TempFile::name`, so this crate puts it back from the media type of the upload.

```rust
use rocket::{form::Form, fs::TempFile, post};
use rocket_download_response::DownloadResponsePro;

#[post("/", data = "<file>")]
fn echo(file: Form<TempFile<'_>>) -> DownloadResponsePro<'_> {
    DownloadResponsePro::from_temp_file(file.into_inner())
}
```

`inline` asks the client to display the body instead of saving it.

```rust
use rocket::get;
use rocket_download_response::DownloadResponse;

#[get("/")]
fn index() -> DownloadResponse {
    DownloadResponse::from_vec(b"Hello world!".to_vec()).file_name("hello.txt").inline()
}
```

## Crates.io

https://crates.io/crates/rocket-download-response

## Documentation

https://docs.rs/rocket-download-response

## License

[MIT](LICENSE)