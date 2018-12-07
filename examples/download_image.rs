#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;

extern crate rocket_download_response;

use std::path::Path;

use rocket_download_response::DownloadResponse;

#[get("/")]
fn download() -> DownloadResponse<'static> {
    let path = Path::join(Path::new("examples"), Path::join(Path::new("images"), "image(貓).jpg"));

    DownloadResponse::from_file(path, None::<String>, None).unwrap()
}

fn main() {
    rocket::ignite().mount("/", routes![download]).launch();
}