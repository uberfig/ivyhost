use crate::analytics::AnalyticsRequest;

pub trait Conn {
    fn init(&self) -> impl std::future::Future<Output = Result<(), String>> + Send;
    fn new_request(
        &self,
        request: AnalyticsRequest,
    ) -> impl std::future::Future<Output = ()> + Send;
}
