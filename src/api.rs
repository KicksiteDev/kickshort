use rocket::{response::Redirect, serde::json::Json};
use serde::{Deserialize, Serialize};

use crate::link::Link;

#[derive(Serialize, Deserialize)]
pub struct LinkRequest {
    pub url: String,
    pub expires_in: Option<usize>,
}

#[derive(Serialize, Deserialize)]
pub struct Error {
    pub error: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LinkResponse {
    id: i32,
    pub short_url: String,
    expires_at: Option<chrono::NaiveDateTime>,
}

impl From<Link> for LinkResponse {
    fn from(link: Link) -> Self {
        LinkResponse {
            id: link.id,
            short_url: link.redirect_url(),
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

#[derive(Responder)]
pub enum APIRedirect {
    #[response(status = 303)]
    To(Redirect),
    #[response(status = 404)]
    NotFound(Redirect)
}

impl APIRedirect {
    pub fn not_found() -> Self {
        APIRedirect::NotFound(Redirect::to("/static/404.html"))
    }
}

impl From<Link> for APIRedirect {
    fn from(link: Link) -> Self {
        dbg!(link.expired());

        if link.expired() {
            return Self::not_found()
        }

        Self::To(Redirect::to(link.url))
    }
}
