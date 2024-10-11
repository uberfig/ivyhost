use actix_web::{get, web::{self, Data}, Responder};
use lazy_static::lazy_static;
use tera::Tera;

use crate::{config::Config, db::pg::PgConn};

lazy_static! {
    pub static ref TEMPLATES: Tera = {
        let mut tera = match Tera::new("../templates/**/*") {
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

#[get("/path/{other_url:.*}")]
async fn path_view(other_url: web::Path<String>, state: Data<Config>, conn: Data<PgConn>) -> impl Responder {
    let path = format!("/{}", other_url.to_string());
    path
}

pub fn get_routes() -> actix_web::Scope {
    actix_web::web::scope("/analytics")
        .service(path_view)
}
