pub trait Conn {
    fn init(&self) -> impl std::future::Future<Output = Result<(), String>> + Send;
}
