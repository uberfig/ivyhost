use serde::Serialize;

use crate::analytics::AnalyticsRequest;

pub struct Path {
    pub path: String,
    pub total_unique: i64,
    pub total_req: i64,
}

#[derive(Serialize, Debug)]
pub struct Graphnode {
    pub amount: u32,
    pub timestamp_start: i64,
    pub timestamp_end: i64
}

#[derive(Serialize, Debug)]
pub struct GraphView {
    pub timeline: Vec<Graphnode>,
    pub title: String,
}

pub trait Conn {
    fn init(&self) -> impl std::future::Future<Output = Result<(), String>> + Send;
    fn new_request(
        &self,
        request: AnalyticsRequest,
    ) -> impl std::future::Future<Output = ()> + Send;
    fn get_paths_alphabetic(&self, limit: i64, ofset: i64) -> impl std::future::Future<Output = Vec<Path>> + Send;
    async fn get_paths_unique_dec(&self, limit: i64, ofset: i64) -> Vec<Path>;
    async fn get_graph(&self, path: &str, title: String, duration: i64, limit: i64, ofset: i64) -> GraphView;
    async fn get_pid(&self, path: &str) -> Option<i64>;
}
