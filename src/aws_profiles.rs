use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::PathBuf;

/// Collect AWS profile names from credentials and config files.
pub fn discover_profiles() -> Vec<String> {
    let mut profiles = Vec::new();
    let mut seen = HashSet::new();

    let mut add_profile = |name: String| {
        if !name.is_empty() && seen.insert(name.clone()) {
            profiles.push(name);
        }
    };

    for path in credentials_paths() {
        if let Ok(contents) = fs::read_to_string(&path) {
            for profile in parse_profile_file(&contents, false) {
                add_profile(profile);
            }
        }
    }

    for path in config_paths() {
        if let Ok(contents) = fs::read_to_string(&path) {
            for profile in parse_profile_file(&contents, true) {
                add_profile(profile);
            }
        }
    }

    profiles
}

fn credentials_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(custom) = env::var("AWS_SHARED_CREDENTIALS_FILE") {
        if !custom.trim().is_empty() {
            paths.push(PathBuf::from(custom));
        }
    }

    if let Some(home) = home_dir() {
        let default_path = home.join(".aws").join("credentials");
        paths.push(default_path);
    }

    paths
}

fn config_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(custom) = env::var("AWS_CONFIG_FILE") {
        if !custom.trim().is_empty() {
            paths.push(PathBuf::from(custom));
        }
    }

    if let Some(home) = home_dir() {
        let default_path = home.join(".aws").join("config");
        paths.push(default_path);
    }

    paths
}

fn home_dir() -> Option<PathBuf> {
    if let Ok(home) = env::var("HOME") {
        if !home.is_empty() {
            return Some(PathBuf::from(home));
        }
    }
    if let Ok(home) = env::var("USERPROFILE") {
        if !home.is_empty() {
            return Some(PathBuf::from(home));
        }
    }
    None
}

fn parse_profile_file(contents: &str, is_config: bool) -> Vec<String> {
    contents
        .lines()
        .filter_map(|line| extract_section_name(line, is_config))
        .collect()
}

fn extract_section_name(line: &str, is_config: bool) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
        return None;
    }

    let name = trimmed.trim_start_matches('[').trim_end_matches(']').trim();
    if name.is_empty() {
        return None;
    }

    if is_config {
        if name.eq_ignore_ascii_case("default") {
            return Some("default".to_string());
        }
        if let Some(rest) = name.strip_prefix("profile") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return None;
            }
            return Some(trimmed.to_string());
        }
        return None;
    }

    Some(name.to_string())
}
