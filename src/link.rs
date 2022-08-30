use base32::{self, Alphabet};
use diesel::{self, prelude::*};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::DbConn;

use self::schema::links;

#[derive(Queryable, Insertable, Serialize, Deserialize, Clone)]
pub struct Link {
    pub id: i32,
    pub url: String,
    pub hash: String,
    pub expires_at: Option<chrono::NaiveDateTime>,
    created_at: chrono::NaiveDateTime,
}

impl Link {
    pub async fn insert(url: String, expires_in: Option<usize>, conn: &DbConn) -> LinkResult {
        conn.run(move |c| {
            let trimmed_url = url.trim_end_matches('/').to_string();
            let expires_at = expires_in.map(|seconds| {
                chrono::Utc::now().naive_utc() + chrono::Duration::seconds(seconds as i64)
            });
            let hash = hash_url(&trimmed_url);
            let new_link = NewLink {
                url: trimmed_url,
                hash,
                expires_at,
            };
            let query = diesel::insert_into(links::table).values(&new_link);
            let link = match query.get_result::<Self>(c) {
                Ok(link) => link,
                Err(_) => return Err("Link not found".to_string()),
            };
            let errors = link.validate();

            if !errors.is_empty() {
                return Err(errors.join(", "));
            }

            Ok(link)
        })
        .await
    }

    pub async fn find_by_hash(hash: String, conn: &DbConn) -> LinkResult {
        conn.run(move |c| {
            let link = match links::table.filter(links::hash.eq(hash)).first::<Self>(c) {
                Ok(link) => link,
                Err(_) => return Err("Link not found".to_string()),
            };

            Ok(link)
        })
        .await
    }

    pub fn expired(&self) -> bool {
        self.expires_at.is_some() && self.expires_at.unwrap() < chrono::Utc::now().naive_utc()
    }

    fn validate(&self) -> Vec<String> {
        let mut errors = vec![];

        if self.url.is_empty() {
            errors.push("URL cannot be empty".to_string());
        }

        if Url::parse(&self.url).is_err() {
            errors.push("Invalid URL".to_string());
        }

        errors
    }
}

fn hash_url(url: &String) -> String {
    base32::encode(Alphabet::Crockford, url.as_bytes()).to_lowercase()[..8].to_string()
}

#[derive(Serialize, Deserialize, Insertable)]
#[table_name = "links"]
struct NewLink {
    url: String,
    hash: String,
    expires_at: Option<chrono::NaiveDateTime>,
}

pub type LinkResult = Result<Link, String>;

mod schema {
    table! {
        links (id) {
            id -> Int4,
            url -> Varchar,
            hash -> Varchar,
            expires_at -> Nullable<Timestamp>,
            created_at -> Timestamp,
        }
    }
}
