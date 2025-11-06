use std::time::Duration;

use async_trait::async_trait;
use aws_config::BehaviorVersion;
use aws_sdk_cloudwatchlogs::types::QueryStatus;
use aws_sdk_cloudwatchlogs::Client;
use aws_types::region::Region;
use tokio::time::sleep;

use super::{LogFetcher, LogField, LogRecord, QueryOutcome, QueryParams};

#[derive(Clone)]
pub struct AwsLogFetcher {
    behavior: BehaviorVersion,
}

impl AwsLogFetcher {
    pub fn new(behavior: BehaviorVersion) -> Self {
        Self { behavior }
    }
}

#[async_trait]
impl LogFetcher for AwsLogFetcher {
    async fn run_query(&self, params: QueryParams) -> QueryOutcome {
        let mut loader = aws_config::defaults(self.behavior);
        if let Some(profile) = params.profile.as_deref() {
            loader = loader.profile_name(profile);
        }
        loader = loader.region(Region::new(params.region.clone()));
        let config = loader.load().await;
        let client = Client::new(&config);

        let log_groups = vec![params.log_group.clone()];
        let joined = log_groups.join(",");

        let start_result = client
            .start_query()
            .log_group_names(joined)
            .query_string(params.query.clone())
            .start_time(params.start_epoch)
            .end_time(params.end_epoch)
            .send()
            .await;

        let start_response = match start_result {
            Ok(resp) => resp,
            Err(err) => {
                return QueryOutcome::Error(format!("Failed to start query: {err:?}"));
            }
        };

        let query_id = match start_response.query_id() {
            Some(id) => id.to_string(),
            None => return QueryOutcome::Error("Missing query id".into()),
        };

        loop {
            match client
                .get_query_results()
                .query_id(query_id.clone())
                .send()
                .await
            {
                Ok(resp) => match resp.status() {
                    Some(QueryStatus::Complete) => {
                        let mut records = Vec::new();
                        for row in resp.results() {
                            let record = row
                                .iter()
                                .map(|field| LogField {
                                    name: field.field().map(|s| s.to_string()),
                                    value: field.value().unwrap_or_default().to_string(),
                                })
                                .collect::<LogRecord>();
                            records.push(record);
                        }
                        return QueryOutcome::Success(records);
                    }
                    Some(QueryStatus::Failed) => {
                        return QueryOutcome::Error("Query failed".into());
                    }
                    Some(QueryStatus::Cancelled) => {
                        return QueryOutcome::Error("Query cancelled".into());
                    }
                    _ => {
                        sleep(Duration::from_millis(500)).await;
                    }
                },
                Err(err) => {
                    return QueryOutcome::Error(format!("Failed to poll query results: {err:?}"));
                }
            }
        }
    }
}
