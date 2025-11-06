use chrono::{Days, Local};

pub struct AppDefaults {
    pub from: String,
    pub to: String,
    pub log_group: &'static str,
    pub query: &'static str,
}

const DEFAULT_QUERY: &str = r#"fields @timestamp, @message, @@m
      | filter @logStream like 'regreport'
      | sort @timestamp asc
      | limit 1000"#;

pub fn default_app_values() -> AppDefaults {
    let from = Local::now()
        .checked_sub_days(Days::new(1))
        .unwrap_or_default();
    let to = Local::now();

    AppDefaults {
        from: from.format("%Y-%m-%d %H:%M:%S").to_string(),
        to: to.format("%Y-%m-%d %H:%M:%S").to_string(),
        log_group: "devg",
        query: DEFAULT_QUERY,
    }
}
