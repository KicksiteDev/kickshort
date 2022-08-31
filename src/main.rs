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
async fn new(link_data: Json<LinkRequest>, conn: DbConn) -> APIResult {
    let url = link_data.url.trim_end_matches('/').to_string();
    let expires_in = link_data.expires_in;

    match Link::insert(url, expires_in, &conn).await {
        Ok(link) => APIResult::created(link),
        Err(e) => APIResult::unprocessable_entity(e),
    }
}

#[get("/<hash>")]
async fn redirect(hash: String, conn: DbConn) -> APIRedirect {
    let link = Link::find_by_hash(hash, &conn).await;

    match link {
        Ok(link) => APIRedirect::from(link),
        Err(_) => APIRedirect::not_found(),
    }
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
        .register(
            "/api/links",
            catchers![unprocessable_entity, bad_request, internal_server_error],
        )
}

#[cfg(test)]
mod test {
    use crate::api::Error;
    use crate::api::LinkResponse;

    use super::rocket;
    use super::Link;
    use rocket::http::{Header, Status};
    use rocket::local::asynchronous::Client;

    static DB_LOCK: parking_lot::Mutex<()> = parking_lot::const_mutex(());

    macro_rules! run_test {
        (|$client:ident, $conn:ident| $block:expr) => {{
            let _lock = DB_LOCK.lock();

            rocket::async_test(async move {
                let $client = Client::tracked(super::rocket())
                    .await
                    .expect("Rocket client");
                let db = super::DbConn::get_one($client.rocket()).await;
                let $conn = db.expect("failed to get database connection for testing");

                Link::delete_all(&$conn).await.expect("failed to delete links");

                $block
            })
        }};
    }

    #[test]
    fn invalid_expiration_date() {
        run_test!(|client, conn| {
            let response = client
                .post("/api/links")
                .header(Header::new("Content-Type", "application/json"))
                .body(r#"{"url": "https://www.google.com", "expires_in": -1 }"#)
                .dispatch()
                .await;
            assert_eq!(response.status(), Status::UnprocessableEntity);
            assert_eq!(response.into_json::<Error>().await.unwrap().error, "Unprocessable Entity; A data type is most likely wrong");
        })
    }

    #[test]
    fn invalid_url() {
        run_test!(|client, conn| {
            let response = client
                .post("/api/links")
                .header(Header::new("Content-Type", "application/json"))
                .body(r#"{"url": "invalid url", "expires_in": 15 }"#)
                .dispatch()
                .await;
            assert_eq!(response.status(), Status::UnprocessableEntity);
            assert_eq!(response.into_json::<Error>().await.unwrap().error, "Invalid URL");
        })
    }

    #[test]
    fn blank_url() {
        run_test!(|client, conn| {
            let response = client
                .post("/api/links")
                .header(Header::new("Content-Type", "application/json"))
                .body(r#"{"url": "", "expires_in": 15 }"#)
                .dispatch()
                .await;
            assert_eq!(response.status(), Status::UnprocessableEntity);
            assert_eq!(response.into_json::<Error>().await.unwrap().error, "URL cannot be empty");
        })
    }

    #[test]
    fn redirect_not_expired() {
        run_test!(|client, _conn| {
            let response = client
                .post("/api/links")
                .header(Header::new("Content-Type", "application/json"))
                .body(r#"{"url": "https://www.google.com", "expires_in": 1000 }"#)
                .dispatch()
                .await;

            assert_eq!(response.status(), Status::Created);

            let hash = response.into_json::<LinkResponse>().await.unwrap().short_url.replace(
                dotenv!("WHO_AM_I"),
                ""
            );

            let response = client.get(hash).dispatch().await;

            assert_eq!(response.status(), Status::SeeOther);
        })
    }

    #[test]
    fn redirect_expired() {
        run_test!(|client, conn| {
            let response = client
                .post("/api/links")
                .header(Header::new("Content-Type", "application/json"))
                .body(r#"{"url": "https://www.google.com" }"#)
                .dispatch()
                .await;

            assert_eq!(response.status(), Status::Created);

            let hash = response.into_json::<LinkResponse>().await.unwrap().short_url.replace(
                dotenv!("WHO_AM_I"),
                ""
            );

            let mut link = Link::find_by_hash(hash[1..].to_string(), &conn).await.unwrap();
            link.expires_at = Some(chrono::Utc::now().naive_utc() - chrono::Duration::seconds(100));
            link.save(&conn).await.unwrap();

            let response = client.get(hash).dispatch().await;

            assert_eq!(response.status(), Status::NotFound);
        })
    }
}
