use std::{
    io::{Cursor, ErrorKind},
    path::Path,
};

use rocket::{
    form::Form,
    fs::TempFile,
    get,
    http::{ContentType, Status},
    local::blocking::Client,
    post, routes,
};
use rocket_download_response::{DownloadResponse, DownloadResponsePro, mime};

const BOUNDARY: &str = "X-BOUNDARY";

#[get("/vec")]
fn vec_page() -> DownloadResponse {
    DownloadResponse::from_vec(b"Hello world!".to_vec()).file_name("hello.txt")
}

#[get("/vec-without-name")]
fn vec_without_name_page() -> DownloadResponse {
    DownloadResponse::from_vec(b"Hello world!".to_vec())
}

#[get("/vec-with-empty-name")]
fn vec_with_empty_name_page() -> DownloadResponse {
    DownloadResponse::from_vec(b"Hello world!".to_vec()).file_name("")
}

#[get("/slice")]
fn slice_page() -> DownloadResponse {
    DownloadResponse::from_slice(b"Hello world!").file_name("hello.txt")
}

#[get("/reader")]
fn reader_page() -> DownloadResponse {
    DownloadResponse::from_reader(Cursor::new(b"Hello world!")).file_name("hello.txt")
}

#[get("/reader-sized")]
fn reader_sized_page() -> DownloadResponse {
    DownloadResponse::from_reader_sized(Cursor::new(b"Hello world!"), 12).file_name("hello.txt")
}

#[get("/vec-with-content-type")]
fn vec_with_content_type_page() -> DownloadResponse {
    DownloadResponse::from_vec(b"Hello world!".to_vec())
        .file_name("hello.jpg")
        .content_type(mime::TEXT_PLAIN)
}

#[get("/inline")]
fn inline_page() -> DownloadResponse {
    DownloadResponse::from_vec(b"Hello world!".to_vec()).file_name("hello.txt").inline()
}

#[get("/apostrophe")]
fn apostrophe_page() -> DownloadResponse {
    DownloadResponse::from_vec(b"Hello world!".to_vec()).file_name("it's.txt")
}

#[get("/file?<file_name>")]
async fn file_page(file_name: Option<&str>) -> Result<DownloadResponse, Status> {
    let path = Path::join(Path::new("examples"), Path::join(Path::new("images"), "image(貓).jpg"));

    let response = DownloadResponse::from_file(path).await.map_err(|err| {
        if err.kind() == ErrorKind::NotFound {
            Status::NotFound
        } else {
            Status::InternalServerError
        }
    })?;

    Ok(match file_name {
        Some(file_name) => response.file_name(file_name),
        None => response,
    })
}

#[post("/echo", data = "<file>")]
async fn echo_page(file: Form<TempFile<'_>>) -> Result<DownloadResponsePro<'_>, Status> {
    DownloadResponsePro::from_temp_file(file.into_inner())
        .await
        .map_err(|_| Status::InternalServerError)
}

fn client() -> Client {
    Client::tracked(rocket::build().mount("/", routes![
        vec_page,
        vec_without_name_page,
        vec_with_empty_name_page,
        vec_with_content_type_page,
        slice_page,
        reader_page,
        reader_sized_page,
        inline_page,
        apostrophe_page,
        file_page,
        echo_page
    ]))
    .unwrap()
}

#[test]
fn vec_with_file_name() {
    let client = client();

    let response = client.get("/vec").dispatch();

    assert_eq!(
        Some("attachment; filename=\"hello.txt\"; filename*=UTF-8''hello.txt"),
        response.headers().get_one("Content-Disposition")
    );
    assert_eq!(Some("text/plain"), response.headers().get_one("Content-Type"));
    assert_eq!(Some(String::from("Hello world!")), response.into_string());
}

/// Without a file name there is still a `Content-Disposition`, otherwise the client would display the body instead of saving it.
#[test]
fn vec_without_file_name() {
    let client = client();

    let response = client.get("/vec-without-name").dispatch();

    assert_eq!(Some("attachment"), response.headers().get_one("Content-Disposition"));
    assert_eq!(Some("application/octet-stream"), response.headers().get_one("Content-Type"));
}

/// An empty file name is how a caller asks for a download that carries no name at all.
#[test]
fn vec_with_empty_file_name() {
    let client = client();

    let response = client.get("/vec-with-empty-name").dispatch();

    assert_eq!(Some("attachment"), response.headers().get_one("Content-Disposition"));
}

#[test]
fn slice_with_file_name() {
    let client = client();

    let response = client.get("/slice").dispatch();

    assert_eq!(
        Some("attachment; filename=\"hello.txt\"; filename*=UTF-8''hello.txt"),
        response.headers().get_one("Content-Disposition")
    );
    assert_eq!(Some("text/plain"), response.headers().get_one("Content-Type"));
    assert_eq!(Some(String::from("Hello world!")), response.into_string());
}

/// The length of a plain reader is not known in advance, so no `Content-Length` can be sent.
#[test]
fn reader_without_content_length() {
    let client = client();

    let response = client.get("/reader").dispatch();

    assert_eq!(None, response.headers().get_one("Content-Length"));
    assert_eq!(Some(String::from("Hello world!")), response.into_string());
}

#[test]
fn reader_with_content_length() {
    let client = client();

    let response = client.get("/reader-sized").dispatch();

    assert_eq!(Some("12"), response.headers().get_one("Content-Length"));
    assert_eq!(Some(String::from("Hello world!")), response.into_string());
}

#[test]
fn explicit_content_type_wins_over_the_extension() {
    let client = client();

    let response = client.get("/vec-with-content-type").dispatch();

    assert_eq!(Some("text/plain"), response.headers().get_one("Content-Type"));
}

#[test]
fn inline_disposition() {
    let client = client();

    let response = client.get("/inline").dispatch();

    assert_eq!(
        Some("inline; filename=\"hello.txt\"; filename*=UTF-8''hello.txt"),
        response.headers().get_one("Content-Disposition")
    );
}

/// `'` is the delimiter of the `filename*` value, so it has to be percent-encoded.
#[test]
fn apostrophe_in_file_name() {
    let client = client();

    let response = client.get("/apostrophe").dispatch();

    assert_eq!(
        Some("attachment; filename=\"it's.txt\"; filename*=UTF-8''it%27s.txt"),
        response.headers().get_one("Content-Disposition")
    );
}

/// `(` and `)` are not in the `attr-char` set of RFC 8187, and neither is any byte of a non-ASCII character.
#[test]
fn file_with_a_name_taken_from_the_path() {
    let client = client();

    let response = client.get("/file").dispatch();

    assert_eq!(Status::Ok, response.status());
    assert_eq!(
        Some("attachment; filename=\"image(_).jpg\"; filename*=UTF-8''image%28%E8%B2%93%29.jpg"),
        response.headers().get_one("Content-Disposition")
    );
    assert_eq!(Some("image/jpeg"), response.headers().get_one("Content-Type"));
}

#[test]
fn renamed_file_uses_the_source_media_type() {
    let client = client();

    let response = client.get("/file?file_name=report.unknownext").dispatch();

    assert_eq!(Status::Ok, response.status());
    assert_eq!(
        Some("attachment; filename=\"report.unknownext\"; filename*=UTF-8''report.unknownext"),
        response.headers().get_one("Content-Disposition")
    );
    assert_eq!(Some("image/jpeg"), response.headers().get_one("Content-Type"));
}

#[test]
fn temp_file_keeps_its_extensions_without_the_client_path() {
    let client = client();

    let body = format!(
        "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; \
         filename=\"C:\\fakepath\\report.final.pdf\"\r\nContent-Type: \
         application/pdf\r\n\r\n%PDF-1.4\r\n--{BOUNDARY}--\r\n"
    );

    let response = client
        .post("/echo")
        .header(ContentType::new("multipart", "form-data").with_params(("boundary", BOUNDARY)))
        .body(body)
        .dispatch();

    assert_eq!(Status::Ok, response.status());
    assert_eq!(
        Some("attachment; filename=\"report.final.pdf\"; filename*=UTF-8''report.final.pdf"),
        response.headers().get_one("Content-Disposition")
    );
    assert_eq!(Some("application/pdf"), response.headers().get_one("Content-Type"));
    assert_eq!(Some("8"), response.headers().get_one("Content-Length"));
    assert_eq!(Some(String::from("%PDF-1.4")), response.into_string());
}

/// A client that does not know what it is uploading sends `application/octet-stream`, so both the extension and the media type have to come from the file name instead.
#[test]
fn temp_file_uploaded_without_a_media_type() {
    let client = client();

    let body = format!(
        "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; \
         filename=\"report.pdf\"\r\nContent-Type: \
         application/octet-stream\r\n\r\n%PDF-1.4\r\n--{BOUNDARY}--\r\n"
    );

    let response = client
        .post("/echo")
        .header(ContentType::new("multipart", "form-data").with_params(("boundary", BOUNDARY)))
        .body(body)
        .dispatch();

    assert_eq!(Status::Ok, response.status());
    assert_eq!(
        Some("attachment; filename=\"report.pdf\"; filename*=UTF-8''report.pdf"),
        response.headers().get_one("Content-Disposition")
    );
    assert_eq!(Some("application/pdf"), response.headers().get_one("Content-Type"));
}

/// A value field of a form arrives as a `TempFile` that is buffered in memory instead of stored on disk, and it carries neither a name nor a media type.
/// Such a body is sent as plain bytes, so its length is left to the server to report just like the one of a `Vec<u8>`.
#[test]
fn buffered_temp_file() {
    let client = client();

    let response =
        client.post("/echo").header(ContentType::Form).body("file=Hello%20world!").dispatch();

    assert_eq!(Status::Ok, response.status());
    assert_eq!(Some("attachment"), response.headers().get_one("Content-Disposition"));
    assert_eq!(Some("application/octet-stream"), response.headers().get_one("Content-Type"));
    assert_eq!(Some(String::from("Hello world!")), response.into_string());
}
