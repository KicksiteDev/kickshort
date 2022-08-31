use diesel::{self, prelude::*};
use serde::{Deserialize, Serialize};
use url::Url;

use rand::distributions::{Alphanumeric, DistString};

use crate::DbConn;

use self::schema::links;

const HASH_FUDGE_LENGTH: usize = 6;
const HASH_LENGTH: usize = 8;

#[derive(Queryable, Insertable, Serialize, Deserialize, Clone, AsChangeset, Identifiable, Debug)]
#[table_name = "links"]
pub struct Link {
    pub id: i32,
    pub url: String,
    pub hash: String,
    pub expires_at: Option<chrono::NaiveDateTime>,
    created_at: chrono::NaiveDateTime,
}

impl Link {
    pub async fn insert(url: String, expires_in: Option<usize>, conn: &DbConn) -> LinkResult {
        let trimmed_url = url.trim_end_matches('/').to_string();
        let expires_at = expires_in.map(|minutes| {
            chrono::Utc::now().naive_utc() + chrono::Duration::minutes(minutes as i64)
        });
        let mut hash = hash_url(&trimmed_url);

        // `hash_url` is pretty much guaranteed to be unique, but on the astronomically rare
        // chance that it isn't, we'll just keep trying
        while Link::find_by_hash(hash.clone(), conn).await.is_ok() {
            hash = hash_url(&trimmed_url);
        }

        let new_link = NewLink {
            url: trimmed_url,
            hash,
            expires_at,
        };

        conn.run(move |c| {
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

    pub async fn delete_all(conn: &DbConn) -> QueryResult<usize> {
        conn.run(|c| diesel::delete(links::table).execute(c)).await
    }

    pub async fn save(self, conn: &DbConn) -> LinkResult {
        conn.run(move |c| match self.save_changes(c) {
            Ok(link) => Ok(link),
            Err(e) => Err(e.to_string()),
        })
        .await
    }

    pub fn redirect_url(&self) -> String {
        let who_am_i = std::env::var("WHO_AM_I").unwrap_or_else(|_| "idk".to_string());

        format!("{}/{}", who_am_i, self.hash)
    }

    pub fn expired(&self) -> bool {
        self.expires_at.is_some() && self.expires_at.unwrap() < chrono::Utc::now().naive_utc()
    }

    fn validate(&self) -> Vec<String> {
        let mut errors = vec![];

        if self.url.is_empty() {
            errors.push("URL cannot be empty".to_string());
        } else if Url::parse(&self.url).is_err() {
            errors.push("Invalid URL".to_string());
        }

        errors
    }
}

fn hash_url(url: &String) -> String {
    let random_fudge = Alphanumeric.sample_string(&mut rand::thread_rng(), HASH_FUDGE_LENGTH);
    let fudged_url = format!("{}{}", random_fudge, url);
    let unsafe_hash = base64_url::encode(&fudged_url);
    let mut url_safe_hash = base64_url::escape(&unsafe_hash).to_string().to_lowercase();
    url_safe_hash.truncate(HASH_LENGTH);

    url_safe_hash
}

#[derive(Serialize, Deserialize, Insertable)]
#[table_name = "links"]
struct NewLink {
    url: String,
    hash: String,
    expires_at: Option<chrono::NaiveDateTime>,
}

pub type LinkResult = Result<Link, String>;

pub mod schema {
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
