use std::{io::ErrorKind, path::Path};

use rocket::{get, http::Status, launch, routes};
use rocket_download_response::DownloadResponse;

#[get("/")]
async fn download() -> Result<DownloadResponse, Status> {
    let path = Path::join(Path::new("examples"), Path::join(Path::new("images"), "image(貓).jpg"));

    DownloadResponse::from_file(path).await.map_err(|err| {
        if err.kind() == ErrorKind::NotFound {
            Status::NotFound
        } else {
            Status::InternalServerError
        }
    })
}

#[launch]
fn rocket() -> _ {
    rocket::build().mount("/", routes![download])
}
