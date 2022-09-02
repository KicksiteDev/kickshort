mod api;
mod cors;
mod link;

#[cfg(test)]
mod tests;

#[macro_use]
extern crate rocket;
#[macro_use]
extern crate rocket_sync_db_pools;
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;

use crate::api::*;
use crate::cors::Cors;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use link::Link;
use map_macro::map;
use rocket::fairing::AdHoc;
use rocket::fs::FileServer;
use rocket::http::Status;
use rocket::response::Redirect;
use rocket::serde::json::Json;
use rocket::{Build, Rocket};
use rocket_dyn_templates::{Template, context};

#[cfg_attr(not(test), database("url_shorten"))]
#[cfg_attr(test, database("url_shorten_test"))]
pub struct DbConn(diesel::PgConnection);

#[cfg(not(test))]
const DATABASE_URL: &str = env!("DATABASE_URL");

#[cfg(not(test))]
const DB_NAME: &str = "url_shorten";

#[cfg(test)]
const DATABASE_URL: &str = env!("DATABASE_URL_TEST");

#[cfg(test)]
const DB_NAME: &str = "url_shorten_test";

#[post("/", data = "<link_data>", format = "application/json")]
async fn new(link_data: Json<LinkRequest>, conn: DbConn) -> APIResult {
    let url = link_data.url.trim_end_matches('/').to_string();
    let expires_in = link_data.expires_in;

    match Link::insert(url, expires_in, &conn).await {
        Ok(link) => APIResult::created(link),
        Err(error) => APIResult::unprocessable_entity(error),
    }
}

#[get("/<hash>")]
async fn redirect(hash: String, conn: DbConn) -> Result<Redirect, Status> {
    let link = match Link::find_by_hash(hash, &conn).await {
        Ok(link) => link,
        Err(_) => return Err(Status::NotFound),
    };

    if link.expired() {
        return Err(Status::NotFound);
    }

    Ok(Redirect::to(link.url))
}

// Intentionally empty, but required for preflight
#[options("/<_..>")]
fn options_all() -> Status {
    Status::Ok
}

#[catch(404)]
fn not_found() -> Template {
    Template::render("404", context! {})
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
    PgConnection::establish(DATABASE_URL)
        .unwrap_or_else(|_| panic!("Error connecting to {}", DATABASE_URL));

    let figment = rocket::Config::figment()
        .merge(("databases", map![DB_NAME => map!("url" => DATABASE_URL)]))
        .merge(("databases", map![DB_NAME => map!("pool_size" => 5)]));

    rocket::custom(figment)
        .attach(Cors)
        .attach(Template::fairing())
        .attach(DbConn::fairing())
        .attach(AdHoc::on_ignite("Run Migrations", run_migrations))
        .mount("/", routes![redirect, options_all])
        .register("/", catchers![not_found])
        .mount("/public", FileServer::from("public"))
        .mount("/api/links", routes![new])
        .register(
            "/api/links",
            catchers![unprocessable_entity, bad_request, internal_server_error],
        )
}
