use std::ops::DerefMut;

use deadpool_postgres::Pool;

use super::conn::Conn;

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
