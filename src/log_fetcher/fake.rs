use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::time::sleep;

use super::{LogFetcher, LogField, LogRecord, QueryOutcome, QueryParams};

#[derive(Clone)]
pub struct FakeLogFetcher {
    records: Arc<Vec<LogRecord>>,
    delay: Duration,
}

impl FakeLogFetcher {
    pub fn new() -> Self {
        Self {
            records: Arc::new(build_fake_records()),
            delay: Duration::from_millis(1500),
        }
    }
}

#[async_trait]
impl LogFetcher for FakeLogFetcher {
    async fn run_query(&self, _params: QueryParams) -> QueryOutcome {
        sleep(self.delay).await;
        QueryOutcome::Success((*self.records).clone())
    }
}

fn build_fake_records() -> Vec<LogRecord> {
    let levels = [
        "Verbose",
        "Debug",
        "Information",
        "Warning",
        "Error",
        "Fatal",
    ];
    let components = [
        "LogBridge.Auth",
        "LogBridge.Billing",
        "LogBridge.Profile",
        "LogBridge.Reporting",
        "LogBridge.Notifications",
        "LogBridge.Edge",
        "LogBridge.Scheduler",
        "LogBridge.Analytics",
    ];
    let templates = [
        "Handled {@Request} for {@User} in {Elapsed}ms",
        "Publishing {@Event} to {Destination}",
        "Retry #{RetryCount} for {@Operation} due to {Reason}",
        "Processing {@Batch} with {RecordCount} records",
        "Cache miss for {@Resource} in shard {Shard}",
        "Persisted {@Entity} version {Version}",
        "Dispatching {@Notification} to {Channel}",
        "Aggregated {@Metric} with {Window} window",
    ];
    let reasons = [
        "Timeout",
        "Throttling",
        "DependencyFailure",
        "ValidationError",
        "ShardReassignment",
        "NetworkGlitch",
        "ColdStart",
        "UnhealthyNode",
    ];

    let mut records = Vec::with_capacity(150);
    for idx in 0..150 {
        let ts = synthetic_timestamp(idx);
        let component = components[idx % components.len()];
        let level = levels[(idx * 7) % levels.len()];
        let template = templates[(idx * 11) % templates.len()];
        let message_template = template;
        let reason = reasons[(idx * 13) % reasons.len()];
        let elapsed = 25 + (idx * 17) % 275;
        let trace_id = format!("{:032x}", (idx as u128 + 1) * 97_531);
        let span_id = format!("{:016x}", (idx as u64 + 1) * 13_957);
        let request_id = format!("req-{idx:05}");
        let user_id = format!("user-{idx:04}");
        let shard = format!("shard-{:02}", (idx * 5) % 32);
        let window = format!("{}m", 5 + (idx % 6) * 5);
        let record_count = 50 + (idx * 23) % 750;
        let retry_count = (idx % 4) + 1;

        let message_body = sorted_compact_json(vec![
            ("@t", ts.clone()),
            ("@mt", message_template.to_string()),
            ("@l", level.to_string()),
            ("@x", format!("System.Exception: Simulated failure in {component}\n   at {component}.Execute()")),
            ("Request", json_compact_object(vec![
                ("Id", request_id.clone()),
                ("Route", format!("/{}/execute", component.replace('.', "/"))),
                ("Method", if idx % 2 == 0 { "POST" } else { "GET" }.to_string()),
                ("Elapsed", elapsed.to_string()),
            ])),
            ("User", json_compact_object(vec![
                ("Id", user_id.clone()),
                ("Region", region_for(idx)),
            ])),
            ("Operation", json_compact_object(vec![
                ("TraceId", trace_id.clone()),
                ("SpanId", span_id.clone()),
                ("RetryCount", retry_count.to_string()),
                ("Reason", reason.to_string()),
            ])),
            ("Metrics", json_compact_object(vec![
                ("Shard", shard.clone()),
                ("Window", window.clone()),
                ("RecordCount", record_count.to_string()),
            ])),
        ]);

        let short_message = format!(
            "{} {} ({} {})",
            component, message_template, request_id, shard
        );

        records.push(vec![
            LogField {
                name: Some("@timestamp".into()),
                value: ts,
            },
            LogField {
                name: Some("@message".into()),
                value: message_body,
            },
            LogField {
                name: Some("@m".into()),
                value: short_message,
            },
        ]);
    }

    records
}

fn synthetic_timestamp(idx: usize) -> String {
    let day = 1 + (idx % 28);
    let hour = (idx * 5) % 24;
    let minute = (idx * 7) % 60;
    let second = (idx * 11) % 60;
    let millis = (idx * 37) % 1000;
    format!(
        "2025-03-{day:02}T{hour:02}:{minute:02}:{second:02}.{:03}Z",
        millis
    )
}

fn json_compact_object(entries: Vec<(&str, String)>) -> String {
    let mut parts = Vec::with_capacity(entries.len());
    for (key, value) in entries {
        let formatted = if is_numeric(&value) {
            value
        } else {
            format!("\"{}\"", escape_json_string(&value))
        };
        parts.push(format!("\"{key}\":{formatted}"));
    }
    format!("{{{}}}", parts.join(","))
}

fn sorted_compact_json(mut kv: Vec<(&str, String)>) -> String {
    kv.sort_by(|a, b| a.0.cmp(b.0));
    let mut parts = Vec::with_capacity(kv.len());
    for (key, value) in kv {
        let trimmed = value.trim();
        let formatted = if looks_like_json(trimmed) {
            trimmed.to_string()
        } else if is_numeric(trimmed) {
            trimmed.to_string()
        } else {
            format!("\"{}\"", escape_json_string(trimmed))
        };
        parts.push(format!("\"{key}\":{formatted}"));
    }
    format!("{{{}}}", parts.join(","))
}

fn looks_like_json(value: &str) -> bool {
    let trimmed = value.trim();
    (trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']'))
}

fn is_numeric(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|c| c.is_ascii_digit())
}

fn escape_json_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn region_for(idx: usize) -> String {
    let regions = ["us-east-1", "us-west-2", "eu-west-1", "ap-southeast-2"];
    regions[idx % regions.len()].to_string()
}
