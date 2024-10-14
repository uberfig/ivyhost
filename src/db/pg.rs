use std::ops::DerefMut;

use deadpool_postgres::Pool;
use tokio_postgres::Row;

use crate::db::conn::Graphnode;

use super::conn::{Conn, GraphView, Path};

mod embedded {
    use refinery::embed_migrations;
    embed_migrations!("./migrations");
}

#[derive(Clone, Debug)]
pub struct PgConn {
    pub db: Pool,
}

impl PgConn {
    async fn select_or_init_visitor(
        transaction: &deadpool_postgres::Transaction<'_>,
        hashed_ip: &str,
    ) -> i64 {
        let stmt = r#"
        SELECT * FROM visitors WHERE ip_address_hash = $1;
        "#;
        let stmt = transaction
            .prepare(stmt)
            .await
            .expect("failed to prepare query");
        let result = transaction
            .query(&stmt, &[&hashed_ip])
            .await
            .expect("failed to get visitor")
            .pop();
        match result {
            Some(x) => x.get("uid"),
            None => {
                let stmt = r#"
                INSERT INTO visitors (ip_address_hash)
                VALUES ($1)
                RETURNING uid;"#;
                let stmt = transaction
                    .prepare(stmt)
                    .await
                    .expect("failed to prepare query");
                transaction
                    .query(&stmt, &[&hashed_ip])
                    .await
                    .expect("failed to insert visitor")
                    .pop()
                    .expect("did not return uid")
                    .get("uid")
            }
        }
    }
    async fn select_or_init_path(
        transaction: &deadpool_postgres::Transaction<'_>,
        path: &str,
    ) -> i64 {
        let stmt = r#"
        SELECT * FROM paths WHERE path = $1;
        "#;
        let stmt = transaction
            .prepare(stmt)
            .await
            .expect("failed to prepare query");
        let result = transaction
            .query(&stmt, &[&path])
            .await
            .expect("failed to get path")
            .pop();
        match result {
            Some(x) => x.get("pid"),
            None => {
                let stmt = r#"
                INSERT INTO paths (path)
                VALUES ($1)
                RETURNING pid;"#;
                let stmt = transaction
                    .prepare(stmt)
                    .await
                    .expect("failed to prepare query");
                transaction
                    .query(&stmt, &[&path])
                    .await
                    .expect("failed to insert path")
                    .pop()
                    .expect("did not return pid")
                    .get("pid")
            }
        }
    }
    async fn incriment_unique(
        transaction: &deadpool_postgres::Transaction<'_>,
        pid: i64,
        uid: i64,
    ) {
        let stmt = r#"
        SELECT uid FROM requests WHERE uid = $1 AND pid = $2 LIMIT 1;
        "#;
        let stmt = transaction
            .prepare(stmt)
            .await
            .expect("failed to prepare query");
        let result = transaction
            .query(&stmt, &[&uid, &pid])
            .await
            .expect("failed to get request")
            .pop();
        match result {
            Some(_) => {}
            None => {
                let stmt = r#"
                UPDATE paths 
                SET unique_visitors = unique_visitors + 1
                WHERE pid = $1;"#;
                let stmt = transaction
                    .prepare(stmt)
                    .await
                    .expect("failed to prepare query");
                transaction
                    .query(&stmt, &[&pid])
                    .await
                    .expect("failed to incriment path unique visitors");
            }
        }
    }
    async fn incriment_total(transaction: &deadpool_postgres::Transaction<'_>, pid: i64) {
        let stmt = r#"
                UPDATE paths 
                SET total_requests = total_requests + 1
                WHERE pid = $1;"#;
        let stmt = transaction
            .prepare(stmt)
            .await
            .expect("failed to prepare query");
        transaction
            .query(&stmt, &[&pid])
            .await
            .expect("failed to incriment path total requests");
    }
    async fn insert_request(
        transaction: &deadpool_postgres::Transaction<'_>,
        pid: i64,
        uid: i64,
        created_at: i64,
    ) {
        let stmt = r#"
                INSERT INTO requests 
                (uid, pid, created_at)
                VALUES ($1, $2, $3);"#;
        let stmt = transaction
            .prepare(stmt)
            .await
            .expect("failed to prepare query");
        transaction
            .query(&stmt, &[&uid, &pid, &created_at])
            .await
            .expect("failed to insert request");
    }
}

impl Conn for PgConn {
    async fn init(&self) -> Result<(), String> {
        init(self).await
    }

    async fn new_request(&self, request: crate::analytics::AnalyticsRequest) {
        let mut client = self.db.get().await.expect("failed to get client");
        let transaction: deadpool_postgres::Transaction<'_> = client
            .transaction()
            .await
            .expect("failed to begin transaction");
        let uid = PgConn::select_or_init_visitor(&transaction, &request.hashed_ip).await;
        let pid = PgConn::select_or_init_path(&transaction, &request.path).await;
        PgConn::incriment_unique(&transaction, pid, uid).await;
        PgConn::incriment_total(&transaction, pid).await;
        PgConn::insert_request(&transaction, pid, uid, request.created_at_milis).await;
        transaction
            .commit()
            .await
            .expect("failed to commit transaction");
    }

    async fn get_total_paths(&self) -> i64 {
        let client = self.db.get().await.expect("failed to get client");
        let stmt = r#"SELECT count(*) as count FROM paths;"#;
        let stmt = client.prepare(stmt).await.expect("failed to prepare query");
        client
            .query(&stmt, &[])
            .await
            .expect("failed to get path count")
            .pop()
            .expect("did not return count")
            .get("count")
    }

    async fn get_paths_alphabetic(&self, limit: i64, ofset: i64) -> Vec<Path> {
        let ofset = ofset * limit;
        let client = self.db.get().await.expect("failed to get client");
        let stmt = r#"
                SELECT * FROM paths 
                ORDER BY path ASC
                LIMIT $1 OFFSET $2;"#;
        let stmt = client.prepare(stmt).await.expect("failed to prepare query");
        client
            .query(&stmt, &[&limit, &ofset])
            .await
            .expect("failed to get paths")
            .iter()
            .map(|x| x.into())
            .collect()
    }

    async fn get_paths_unique_visitors_dec(
        &self,
        limit: i64,
        ofset: i64,
    ) -> Vec<super::conn::Path> {
        let ofset = ofset * limit;
        let client = self.db.get().await.expect("failed to get client");
        let stmt = r#"
                SELECT * FROM paths 
                ORDER BY unique_visitors DESC
                LIMIT $1 OFFSET $2;"#;
        let stmt = client.prepare(stmt).await.expect("failed to prepare query");
        client
            .query(&stmt, &[&limit, &ofset])
            .await
            .expect("failed to get paths")
            .iter()
            .map(|x| x.into())
            .collect()
    }

    async fn get_graph_total(
        &self,
        pid: i64,
        title: String,
        duration: i64,
        limit: usize,
        current_time: i64,
    ) -> super::conn::GraphView {
        let client = self.db.get().await.expect("failed to get client");
        let stmt = r#"SELECT COUNT(*) as count FROM requests WHERE pid = $1 AND created_at <= $2  AND created_at > $3;"#;
        let stmt = client.prepare(stmt).await.expect("failed to prepare query");

        let mut timeline = Vec::<Graphnode>::with_capacity(limit);
        for i in (0..limit as i64).rev() {
            let range_recent = current_time - (duration * i);
            let range_oldest = current_time - (duration * (i + 1));

            let amount: i64 = client
                .query(&stmt, &[&pid, &range_recent, &range_oldest])
                .await
                .expect("failed to get path count")
                .pop()
                .expect("did not return count")
                .get("count");

            timeline.push(Graphnode {
                amount: amount as u32,
                timestamp_start: range_oldest,
                timestamp_end: range_recent,
            });
        }
        GraphView { timeline, title }
    }

    async fn get_graph_unique(
        &self,
        pid: i64,
        title: String,
        duration: i64,
        limit: usize,
        current_time: i64,
    ) -> super::conn::GraphView {
        let client = self.db.get().await.expect("failed to get client");
        let stmt = r#"SELECT COUNT(DISTINCT uid) as count FROM requests WHERE pid = $1 AND created_at <= $2  AND created_at > $3;"#;
        let stmt = client.prepare(stmt).await.expect("failed to prepare query");

        let mut timeline = Vec::<Graphnode>::with_capacity(limit);
        for i in (0..limit as i64).rev() {
            let range_recent = current_time - (duration * i);
            let range_oldest = current_time - (duration * (i + 1));

            let amount: i64 = client
                .query(&stmt, &[&pid, &range_recent, &range_oldest])
                .await
                .expect("failed to get path count")
                .pop()
                .expect("did not return count")
                .get("count");

            timeline.push(Graphnode {
                amount: amount as u32,
                timestamp_start: range_oldest,
                timestamp_end: range_recent,
            });
        }
        GraphView { timeline, title }
    }

    async fn get_pid(&self, path: &str) -> Option<i64> {
        let client = self.db.get().await.expect("failed to get client");
        let stmt = r#"
                SELECT * FROM paths where path = $1;"#;
        let stmt = client.prepare(stmt).await.expect("failed to prepare query");
        client
            .query(&stmt, &[&path])
            .await
            .expect("failed to get path count")
            .pop()
            .map(|x| x.get("pid"))
    }

    async fn get_path(&self, pid: i64) -> Path {
        let client = self.db.get().await.expect("failed to get client");
        let stmt = r#"
                SELECT * FROM paths where pid = $1;"#;
        let stmt = client.prepare(stmt).await.expect("failed to prepare query");
        let result = client
            .query(&stmt, &[&pid])
            .await
            .expect("failed to get path count")
            .pop()
            .expect("pid does not exist");
        Path {
            path: result.get("path"),
            total_unique: result.get("unique_visitors"),
            total_requests: result.get("total_requests"),
        }
    }
}

pub async fn init(conn: &PgConn) -> Result<(), String> {
    let mut conn = conn
        .db
        .get()
        .await
        .expect("could not get conn for migrations");
    let client = conn.deref_mut().deref_mut();
    let report = embedded::migrations::runner().run_async(client).await;
    match report {
        Ok(x) => {
            println!("migrations sucessful");
            if x.applied_migrations().is_empty() {
                println!("no migrations applied")
            } else {
                println!("applied migrations: ");
                for migration in x.applied_migrations() {
                    match migration.applied_on() {
                        Some(x) => println!(" - {} applied {}", migration.name(), x),
                        None => println!(" - {} applied N/A", migration.name()),
                    }
                }
            }
        }
        Err(x) => {
            return Err(x.to_string());
        }
    }
    Ok(())
}

impl From<&Row> for Path {
    fn from(value: &Row) -> Self {
        Path {
            path: value.get("path"),
            total_unique: value.get("unique_visitors"),
            total_requests: value.get("total_requests"),
        }
    }
}
