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

impl Conn for PgConn {
    async fn init(&self) -> Result<(), String> {
        init(self).await
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