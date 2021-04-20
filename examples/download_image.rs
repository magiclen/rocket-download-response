#[macro_use]
extern crate rocket;

extern crate rocket_download_response;

use std::path::Path;

use rocket_download_response::DownloadResponse;

#[get("/")]
fn download() -> DownloadResponse {
    let path = Path::join(Path::new("examples"), Path::join(Path::new("images"), "image(貓).jpg"));

    DownloadResponse::from_file(path, None::<String>, None)
}

#[launch]
fn rocket() -> _ {
    rocket::build().mount("/", routes![download])
}
