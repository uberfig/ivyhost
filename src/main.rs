use actix_files::{self as fs, NamedFile};
use actix_web::{
    dev::{fn_service, ServiceRequest, ServiceResponse},
    http::Error,
    middleware::from_fn,
    post,
    web::Data,
    App, HttpResponse, HttpServer,
};
use git2::Repository;
use ivyhost::{
    analytics::simple_analytics, analytics_routes::get_routes, config::Config, db::conn::Conn, pull::{do_fetch, do_merge}
};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let config = Config::get_config().expect("failed to load config");
    git_refresh(&config.site_repo, &config.branch);
    start_application(config).await
}

pub async fn start_application(config: Config) -> std::io::Result<()> {
    let conn = config.create_conn();
    if let Err(x) = conn.init().await {
        eprintln!("{}", x);
        return Ok(());
    }

    let bind = config.bind_address.clone();
    let port = config.port;

    HttpServer::new(move || {
        App::new()
            .app_data(Data::new(conn.to_owned()))
            .app_data(Data::new(config.to_owned()))
            .service(refresh)
            .service(get_routes())
            .service(
                fs::Files::new("/", "./static/repo/public")
                    .index_file("index.html")
                    .show_files_listing()
                    .default_handler(fn_service(|req: ServiceRequest| async {
                        let (req, _) = req.into_parts();
                        let file = NamedFile::open_async("./static/repo/public/404.html").await?;
                        let res = file.into_response(&req);
                        Ok(ServiceResponse::new(req, res))
                    })),
            )
            .wrap(from_fn(simple_analytics))
    })
    .bind((bind, port))?
    .run()
    .await
}

fn git_refresh(url: &str, branch: &str) {
    let repo = match Repository::open("./static/repo") {
        Ok(repo) => repo,
        Err(_e) => match Repository::clone(url, "./static/repo") {
            Ok(repo) => repo,
            Err(e) => panic!("failed to clone: {}", e),
        },
    };

    //git pull
    let mut remote = repo.find_remote("origin").unwrap();
    let fetch_commit = do_fetch(&repo, &[branch], &mut remote).unwrap();
    let _x = do_merge(&repo, &branch, fetch_commit);
}

#[post("/refresh")]
pub async fn refresh(state: Data<Config>) -> Result<HttpResponse, Error> {
    git_refresh(&state.site_repo, &state.branch);
    Ok(HttpResponse::Ok().body("refreshed"))
}
