use base64::Engine;
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

use actix_web::{
    body::MessageBody,
    dev::{ServiceRequest, ServiceResponse},
    middleware::Next,
    web::Data,
    Error,
};

use crate::{
    config::Config,
    db::{conn::Conn, pg::PgConn},
};

pub struct AnalyticsRequest {
    pub hashed_ip: String,
    pub path: String,
    pub created_at_milis: i64,
}

pub async fn simple_analytics(
    req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> Result<ServiceResponse<impl MessageBody>, Error> {
    let real_ip_header = req
        .app_data::<Data<Config>>()
        .expect("missing config from app data")
        .real_ip_header
        .clone();

    let conn = req
        .app_data::<Data<PgConn>>()
        .expect("missing conn from app data")
        .clone();
    let path = req.path().to_string();
    let ip = req.headers().get(real_ip_header).cloned();
    // pre-processing
    let fut = next.call(req).await;
    // post-processing

    let Some(ip) = ip else {
        return fut;
    };
    let Ok(ip) = ip.to_str() else {
        return fut;
    };
    println!("{}", &ip);

    if let Ok(val) = &fut {
        if val.response().status().is_success() {
            let hashed_ip = sha256_hash(ip.as_bytes());
            let current_time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time went backwards")
                .as_millis() as i64;
            conn.new_request(AnalyticsRequest {
                hashed_ip,
                path,
                created_at_milis: current_time,
            })
            .await;
        }
    }

    fut
}

/// generates an sha256 digest of the provided buffer encoded in base64
pub fn sha256_hash(body: &[u8]) -> String {
    let mut hasher = Sha256::new();
    // write input message
    hasher.update(body);
    let finished = hasher.finalize();
    base64::prelude::BASE64_STANDARD.encode(finished)
}
