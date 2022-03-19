#[macro_use]
extern crate rocket;

use std::io::ErrorKind;
use std::path::Path;

use rocket::http::Status;

use rocket_download_response::DownloadResponse;

#[get("/")]
async fn download() -> Result<DownloadResponse, Status> {
    let path = Path::join(Path::new("examples"), Path::join(Path::new("images"), "image(貓).jpg"));

    DownloadResponse::from_file(path, None::<String>, None).await.map_err(|err| {
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
