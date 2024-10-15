use actix_web::{
    error::ErrorNotFound,
    get,
    web::{self, Data},
    HttpResponse, Responder, Result,
};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use tera::{Context, Tera};

use crate::{
    config::Config,
    db::{conn::Conn, pg::PgConn},
};

lazy_static! {
    pub static ref TEMPLATES: Tera = {
        let mut tera = match Tera::new("templates/**/*.html") {
            Ok(t) => t,
            Err(e) => {
                println!("Parsing error(s): {}", e);
                ::std::process::exit(1);
            }
        };
        tera.autoescape_on(vec![".html", ".sql"]);
        // tera.register_filter("do_nothing", do_nothing_filter);
        tera
    };
}

use std::{
    ops::{Rem, RemAssign},
    time::{Duration, SystemTime},
};

trait DurationExt {
    fn from_mins(mins: u64) -> Duration;
    fn from_days(days: u64) -> Duration;
}

impl DurationExt for Duration {
    fn from_mins(mins: u64) -> Duration {
        Duration::from_secs(mins * 60)
    }

    fn from_days(days: u64) -> Duration {
        Duration::from_secs(days * 60 * 60 * 24)
    }
}

#[get("/path/{other_url:.*}")]
async fn path_view(
    other_url: web::Path<String>,
    // state: Data<Config>,
    conn: Data<PgConn>,
) -> Result<HttpResponse> {
    let path = format!("/{}", other_url.to_string());
    let pid = match conn.get_pid(&path).await {
        Some(pid) => pid,
        None => return Err(ErrorNotFound(format!("{} not found", path))),
    };

    const LIMIT: usize = 40;

    use std::time::Duration;

    let time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;

    let duration = Duration::from_mins(30).as_millis() as i64;
    let half_hourly_total = conn
        .get_graph_total(pid, "Half Hourly".to_string(), duration, LIMIT, time)
        .await;
    let half_hourly_unique = conn
        .get_graph_unique(pid, "Half Hourly".to_string(), duration, LIMIT, time)
        .await;

    let duration = Duration::from_days(1).as_millis() as i64;
    let daily_total = conn
        .get_graph_total(pid, "Daily".to_string(), duration, LIMIT, time)
        .await;
    let daily_unique = conn
        .get_graph_unique(pid, "Daily".to_string(), duration, LIMIT, time)
        .await;

    let duration = Duration::from_days(30).as_millis() as i64;
    let monthly_total = conn
        .get_graph_total(pid, "Monthly (30 days)".to_string(), duration, LIMIT, time)
        .await;
    let monthly_unique = conn
        .get_graph_unique(pid, "Monthly (30 days)".to_string(), duration, LIMIT, time)
        .await;
    let totals = vec![half_hourly_total, daily_total, monthly_total];
    let uniques = vec![half_hourly_unique, daily_unique, monthly_unique];
    let path = conn.get_path(pid).await;

    let mut context = Context::new();
    context.insert("graphs_total", &totals);
    context.insert("graphs_unique", &uniques);
    context.insert("path", &path);

    let val = TEMPLATES
        .render("path.html", &context)
        .expect("tera rendering error");

    Ok(HttpResponse::Ok().body(val))
}

#[derive(Deserialize, Serialize, Debug, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum Ordering {
    Alphabetical,
    Unique,
}

#[derive(Deserialize, Debug)]
struct Info {
    page: Option<u64>,
    order_by: Option<Ordering>,
}
#[get("")]
async fn index(info: web::Query<Info>, conn: Data<PgConn>) -> Result<HttpResponse> {
    const LIMIT: i64 = 20;
    let ordering = info.order_by.unwrap_or(Ordering::Alphabetical);
    let page: i64 = info.page.unwrap_or(0).try_into().unwrap_or(0);
    let total_pages = conn.get_total_paths().await;
    let total_pages = match total_pages.rem(&LIMIT) != 0 {
        true => (total_pages / LIMIT) + 1,
        false => total_pages / LIMIT,
    };

    let routes = match ordering {
        Ordering::Alphabetical => conn.get_paths_alphabetic(LIMIT, page).await,
        Ordering::Unique => conn.get_paths_unique_visitors_dec(LIMIT, page).await,
    };

    let mut context = Context::new();
    context.insert("routes", &routes);
    context.insert("page", &page);
    context.insert("total_pages", &total_pages);
    context.insert("ordering", &ordering);

    let val = TEMPLATES
        .render("index.html", &context)
        .expect("tera rendering error");

    Ok(HttpResponse::Ok().body(val))
}

pub fn get_routes() -> actix_web::Scope {
    actix_web::web::scope("/analytics")
        .service(path_view)
        .service(index)
}
