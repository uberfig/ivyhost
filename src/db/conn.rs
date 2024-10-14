use serde::Serialize;

use crate::analytics::AnalyticsRequest;

#[derive(Serialize, Debug)]
pub struct Path {
    pub path: String,
    pub total_unique: i64,
    pub total_requests: i64,
}

#[derive(Serialize, Debug)]
pub struct Graphnode {
    pub amount: u32,
    pub timestamp_start: i64,
    pub timestamp_end: i64,
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
    fn get_total_paths(&self) -> impl std::future::Future<Output = i64> + Send;
    fn get_paths_alphabetic(
        &self,
        limit: i64,
        ofset: i64,
    ) -> impl std::future::Future<Output = Vec<Path>> + Send;
    fn get_paths_unique_visitors_dec(&self, limit: i64, ofset: i64) -> impl std::future::Future<Output = Vec<Path>> + Send;
    fn get_graph_total(
        &self,
        pid: i64,
        title: String,
        duration: i64,
        limit: usize,
        current_time: i64,
    ) -> impl std::future::Future<Output = GraphView> + Send;
    fn get_graph_unique(
        &self,
        pid: i64,
        title: String,
        duration: i64,
        limit: usize,
        current_time: i64,
    ) -> impl std::future::Future<Output = GraphView> + Send;
    fn get_pid(&self, path: &str) -> impl std::future::Future<Output = Option<i64>> + Send;
    async fn get_path(&self, pid: i64) -> Path;
}
