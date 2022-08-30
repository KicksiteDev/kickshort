use rocket::{response::Redirect, serde::json::Json};
use serde::{Deserialize, Serialize};

use crate::link::Link;

#[derive(Serialize, Deserialize)]
pub struct LinkData {
    pub url: String,
    pub expires_in: Option<usize>,
}

#[derive(Serialize, Deserialize)]
pub struct Error {
    error: String,
}

#[derive(Serialize, Deserialize)]
pub struct LinkResponse {
    id: i32,
    new_url: String,
    expires_at: Option<chrono::NaiveDateTime>,
}

impl From<Link> for LinkResponse {
    fn from(link: Link) -> Self {
        LinkResponse {
            id: link.id,
            new_url: format!("{}/{}", std::env::var("WHO_AM_I").unwrap(), link.hash),
            expires_at: link.expires_at,
        }
    }
}

#[derive(Responder)]
#[allow(dead_code)]
pub enum APIResult {
    #[response(status = 400)]
    BadRequest(Json<Error>),
    #[response(status = 404)]
    NotFound(Json<Error>),
    #[response(status = 500)]
    InternalServerError(Json<Error>),
    #[response(status = 422)]
    UnprocessableEntity(Json<Error>),
    #[response(status = 201)]
    Created(Json<LinkResponse>),
    #[response(status = 200)]
    Ok(Json<LinkResponse>),
}

#[allow(dead_code)]
impl APIResult {
    pub fn bad_request(error: String) -> Self {
        APIResult::BadRequest(Json(Error { error }))
    }
    pub fn not_found(error: String) -> Self {
        APIResult::NotFound(Json(Error { error }))
    }
    pub fn internal_server_error(error: String) -> Self {
        APIResult::InternalServerError(Json(Error { error }))
    }
    pub fn unprocessable_entity(error: String) -> Self {
        APIResult::UnprocessableEntity(Json(Error { error }))
    }
    pub fn created(link: Link) -> Self {
        APIResult::Created(Json(LinkResponse::from(link)))
    }
    pub fn ok(link: Link) -> Self {
        APIResult::Ok(Json(LinkResponse::from(link)))
    }
}

pub struct APIRedirect;

impl APIRedirect {
    pub fn to(link: Result<Link, String>) -> Redirect {
        match link {
            Ok(link) => Redirect::to(format!(
                "{}/{}",
                std::env::var("WHO_AM_I").unwrap(),
                link.hash
            )),
            Err(_) => Redirect::to("/static/404.html"),
        }
    }
}
