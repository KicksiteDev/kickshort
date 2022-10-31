use diesel::{self, prelude::*};
use serde::{Deserialize, Serialize};
use url::Url;

use rand::distributions::{Alphanumeric, DistString};

use crate::{
    paginate::Paginate,
    DbConn,
};

use self::schema::links;

const HASH_FUDGE_LENGTH: usize = 6;
const HASH_LENGTH: usize = 8;

#[derive(
    Queryable, Insertable, Serialize, Deserialize, Clone, AsChangeset, Identifiable, Debug,
)]
#[table_name = "links"]
pub struct Link {
    pub id: i32,
    pub url: String,
    pub hash: String,
    pub visible: bool,
    pub visitors: i32,
    pub created_at: chrono::NaiveDateTime,
    pub title: Option<String>,
}

impl Link {
    pub async fn paginate(
        conn: &DbConn,
        page: i64,
        per_page: i64,
    ) -> Result<(Vec<Link>, i64), diesel::result::Error> {
        // load all where visiblity true
        conn.run(move |c| {
            links::table
                .filter(links::visible.eq(true))
                .order(links::created_at.desc())
                .paginate(page)
                .per_page(per_page)
                .load_and_count_pages(c)
        })
        .await
    }

    pub async fn insert(
        url: String,
        visible: bool,
        custom_hash: Option<String>,
        title: Option<String>,
        conn: &DbConn,
    ) -> LinkResult {
        let trimmed_url = url.trim_end_matches('/').to_string();

        let hash = match custom_hash.clone() {
            Some(hash) => {
                let link = Link::find_by_hash(hash.clone(), conn).await;
                if link.is_ok() {
                    return Err("URL has already been taken".to_string());
                }

                hash
            }
            None => {
                let mut hash = hash_url(&trimmed_url);

                while Link::find_by_hash(hash.clone(), conn).await.is_ok() {
                    hash = hash_url(&trimmed_url);
                }

                hash
            }
        };

        let new_link = NewLink {
            url: trimmed_url,
            hash,
            visible,
            title,
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

    pub async fn find(id: i32, conn: &DbConn) -> LinkResult {
        conn.run(move |c| {
            links::table
                .find(id)
                .get_result::<Self>(c)
                .map_err(|_| "Link not found".to_string())
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

    pub async fn increment_visitors(self, conn: &DbConn) -> bool {
        conn.run(move |c| {
            diesel::update(&self)
                .set(links::visitors.eq(links::visitors + 1))
                .execute(c)
                .is_ok()
        })
        .await
    }

    pub async fn delete(self, conn: &DbConn) -> bool {
        conn.run(move |c| diesel::delete(&self).execute(c).is_ok())
            .await
    }

    pub async fn delete_all(conn: &DbConn) -> QueryResult<usize> {
        conn.run(move |c| diesel::delete(links::table).execute(c))
            .await
    }

    pub async fn save(self, conn: &DbConn) -> LinkResult {
        conn.run(move |c| match self.save_changes(c) {
            Ok(link) => Ok(link),
            Err(e) => Err(e.to_string()),
        })
        .await
    }

    pub fn redirect_url(&self) -> String {
        let who_am_i = std::env::var("WHO_AM_I").expect("WHO_AM_I must be set");

        format!("{}/{}", who_am_i, self.hash)
    }

    fn validate(&self) -> Vec<String> {
        let mut errors = vec![];

        if self.url.is_empty() {
            errors.push("URL cannot be empty".to_string());
        } else if Url::parse(&self.url).is_err() {
            errors.push("Invalid URL".to_string());
        }

        if let Some(title) = &self.title {
            if title.len() > 255 {
                errors.push("Title cannot be over 255 characters".to_string());
            }
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
    visible: bool,
    title: Option<String>,
}

pub type LinkResult = Result<Link, String>;

pub mod schema {
    table! {
        links (id) {
            id -> Int4,
            url -> Varchar,
            hash -> Varchar,
            visible -> Bool,
            visitors -> Int4,
            created_at -> Timestamp,
            title -> Nullable<Varchar>,
        }
    }
}
