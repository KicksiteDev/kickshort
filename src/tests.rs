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
            env!("WHO_AM_I"),
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
            env!("WHO_AM_I"),
            ""
        );

        let mut link = Link::find_by_hash(hash[1..].to_string(), &conn).await.unwrap();
        link.expires_at = Some(chrono::Utc::now().naive_utc() - chrono::Duration::seconds(100));
        link.save(&conn).await.unwrap();

        let response = client.get(hash).dispatch().await;

        assert_eq!(response.status(), Status::NotFound);
    })
}
