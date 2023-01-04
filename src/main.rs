extern crate rocket;

use rocket::{get, launch, routes};

#[get("/")]
async fn index() -> String {
    return "Hello, world!".to_string();
}

#[launch]
fn rocket() -> _ {
    rocket::build().mount("/", routes![index])
}
