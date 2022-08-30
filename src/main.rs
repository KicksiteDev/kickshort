mod api;
mod link;

#[macro_use]
extern crate rocket;
#[macro_use]
extern crate rocket_sync_db_pools;
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;
#[macro_use]
extern crate dotenv_codegen;

use crate::api::*;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use dotenv::dotenv;
use link::Link;
use map_macro::map;
use rocket::fairing::AdHoc;
use rocket::fs::FileServer;
use rocket::response::Redirect;
use rocket::serde::json::Json;
use rocket::{Build, Rocket};

#[cfg_attr(not(test), database("url_shorten"))]
#[cfg_attr(test, database("url_shorten_test"))]
pub struct DbConn(diesel::PgConnection);

#[cfg(not(test))]
const DATABASE_URL: &str = dotenv!("DATABASE_URL");

#[cfg(not(test))]
const DB_NAME: &str = "url_shorten";

#[cfg(test)]
const DATABASE_URL: &str = dotenv!("DATABASE_URL_TEST");

#[cfg(test)]
const DB_NAME: &str = "url_shorten_test";

#[post("/", data = "<link_data>", format = "application/json")]
async fn new(link_data: Json<LinkData>, conn: DbConn) -> APIResult {
    let url = link_data.url.trim_end_matches('/').to_string();
    let expires_in = link_data.expires_in;

    match Link::insert(url, expires_in, &conn).await {
        Ok(link) => APIResult::created(link),
        Err(e) => APIResult::unprocessable_entity(e),
    }
}

#[get("/<hash>")]
async fn redirect(hash: String, conn: DbConn) -> Redirect {
    let link = Link::find_by_hash(hash, &conn).await;

    APIRedirect::to(link)
}

#[get("/")]
fn index() -> Redirect {
    Redirect::to("/static/404.html")
}

#[catch(404)]
fn not_found() -> Redirect {
    Redirect::to("/static/404.html")
}

#[catch(400)]
fn bad_request() -> APIResult {
    APIResult::bad_request("Bad request".to_string())
}

#[catch(500)]
fn internal_server_error() -> APIResult {
    APIResult::internal_server_error("Internal server error".to_string())
}

#[catch(422)]
fn unprocessable_entity() -> APIResult {
    // TODO: This catches when you pass the wrong type of data to the API
    //       There should maybe be a better way to do this
    APIResult::unprocessable_entity(
        "Unprocessable Entity; A data type is most likely wrong".to_string(),
    )
}

async fn run_migrations(rocket: Rocket<Build>) -> Rocket<Build> {
    embed_migrations!();

    let conn = DbConn::get_one(&rocket).await.expect("database connection");
    conn.run(|c| embedded_migrations::run(c))
        .await
        .expect("can run migrations");

    rocket
}

#[launch]
fn rocket() -> _ {
    dotenv().ok();

    PgConnection::establish(DATABASE_URL)
        .unwrap_or_else(|_| panic!("Error connecting to {}", DATABASE_URL));

    let figment = rocket::Config::figment()
        .merge(("databases", map![DB_NAME => map!("url" => DATABASE_URL)]));

    rocket::custom(figment)
        .attach(DbConn::fairing())
        .attach(AdHoc::on_ignite("Run Migrations", run_migrations))
        .mount("/", routes![redirect, index])
        .register("/", catchers![not_found])
        .mount("/static", FileServer::from("static"))
        .mount("/api/links", routes![new])
        .register("/api/links", catchers![unprocessable_entity, bad_request, internal_server_error])
}

// TODO: Technically these stop working with more tests,
// because they run in parallel and connect to the same database
// so it runs out of clients
#[cfg(test)]
mod test {
    use super::rocket;
    use rocket::http::{Header, Status};
    use rocket::local::blocking::Client;

    #[test]
    fn new_invalid() {
        let client = Client::tracked(rocket()).expect("valid rocket instance");
        let response = client
            .post("/api/links")
            .header(Header::new("Content-Type", "application/json"))
            .body(r#"{"url": "invalid-gaming"}"#)
            .dispatch();
        assert_eq!(response.status(), Status::UnprocessableEntity);
    }

    #[test]
    fn new_valid() {
        let client = Client::tracked(rocket()).expect("valid rocket instance");
        let response = client
            .post("/api/links")
            .header(Header::new("Content-Type", "application/json"))
            .body(r#"{"url": "https://www.google.com" }"#).dispatch();
        assert_eq!(response.status(), Status::Created);
    }
}
