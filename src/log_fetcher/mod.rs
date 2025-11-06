use async_trait::async_trait;

pub mod aws;
pub mod fake;

pub use aws::AwsLogFetcher;
pub use fake::FakeLogFetcher;

#[derive(Clone)]
pub struct QueryParams {
    pub start_epoch: i64,
    pub end_epoch: i64,
    pub log_group: String,
    pub query: String,
    pub region: String,
    pub profile: Option<String>,
}

#[derive(Clone)]
pub struct LogField {
    pub name: Option<String>,
    pub value: String,
}

pub type LogRecord = Vec<LogField>;

pub enum QueryOutcome {
    Success(Vec<LogRecord>),
    Error(String),
}

#[async_trait]
pub trait LogFetcher: Send + Sync {
    async fn run_query(&self, params: QueryParams) -> QueryOutcome;
}
