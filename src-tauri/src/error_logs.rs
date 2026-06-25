use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tauri::Manager;
use tauri_plugin_log::{Target, TargetKind, WEBVIEW_TARGET, fern};
use time::{Date, Month, OffsetDateTime};

const ERROR_LOG_DIR_NAME: &str = "error-logs";
const RETENTION_DAYS: i64 = 14;

static ERROR_LOG_DIR: OnceLock<PathBuf> = OnceLock::new();

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorLogDay {
    pub date: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorLogRead {
    pub date: String,
    pub content: String,
    pub line_count: usize,
}

pub fn configure(app_handle: &tauri::AppHandle) -> Result<(), String> {
    let dir = error_log_dir(app_handle)?;
    let _ = ERROR_LOG_DIR.set(dir);
    Ok(())
}

pub fn daily_error_target() -> Target {
    let dispatch = fern::Dispatch::new()
        .filter(|metadata| {
            metadata.level() == log::Level::Error && metadata.target() != WEBVIEW_TARGET
        })
        .chain(fern::Output::call(|record| {
            append_backend_record(record);
        }));
    Target::new(TargetKind::Dispatch(dispatch))
}

pub fn list_days(app_handle: &tauri::AppHandle) -> Result<Vec<ErrorLogDay>, String> {
    list_days_from_dir(&error_log_dir(app_handle)?).map_err(|error| error.to_string())
}

pub fn read_day(app_handle: &tauri::AppHandle, date: &str) -> Result<ErrorLogRead, String> {
    read_day_from_dir(&error_log_dir(app_handle)?, date).map_err(|error| error.to_string())
}

pub fn record_frontend_error(
    app_handle: &tauri::AppHandle,
    source: &str,
    message: &str,
    stack: Option<&str>,
) -> Result<(), String> {
    let dir = error_log_dir(app_handle)?;
    let now = OffsetDateTime::now_utc();
    let date = date_string(now.date());
    let mut line = format!(
        "{}[frontend:{}][ERROR] {}",
        timestamp_string(now),
        sanitize_source(source),
        sanitize_message(message)
    );
    if let Some(stack) = stack.and_then(non_empty_trimmed) {
        line.push('\n');
        line.push_str(&sanitize_stack(stack));
    }
    append_error_record(&dir, &date, &line).map_err(|error| error.to_string())?;
    prune_old_logs_from_dir(&dir, &date).map_err(|error| error.to_string())
}

fn error_log_dir(app_handle: &tauri::AppHandle) -> Result<PathBuf, String> {
    app_handle
        .path()
        .app_log_dir()
        .map(|dir| dir.join(ERROR_LOG_DIR_NAME))
        .map_err(|error| format!("no log dir: {}", error))
}

fn append_backend_record(record: &log::Record<'_>) {
    let Some(dir) = ERROR_LOG_DIR.get() else {
        return;
    };
    let now = OffsetDateTime::now_utc();
    let date = date_string(now.date());
    let target = crate::plugin_engine::host_api::redact_log_message(record.target());
    let message = crate::plugin_engine::host_api::redact_log_message(&record.args().to_string());
    let line = format!(
        "{}[{}][{}] {}",
        timestamp_string(now),
        target,
        record.level(),
        message
    );
    if let Err(error) =
        append_error_record(dir, &date, &line).and_then(|_| prune_old_logs_from_dir(dir, &date))
    {
        eprintln!("failed to write OpenUsage error log: {}", error);
    }
}

fn append_error_record(dir: &Path, date: &str, record: &str) -> std::io::Result<()> {
    validate_date(date)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;
    fs::create_dir_all(dir)?;
    let path = dir.join(format!("{}.log", date));
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{}", record.trim_end())?;
    Ok(())
}

fn list_days_from_dir(dir: &Path) -> std::io::Result<Vec<ErrorLogDay>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut days = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(date) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        if validate_date(date).is_err()
            || path.extension().and_then(|value| value.to_str()) != Some("log")
        {
            continue;
        }
        let content = fs::read_to_string(&path).unwrap_or_default();
        days.push(ErrorLogDay {
            date: date.to_string(),
            count: count_entries(&content),
        });
    }
    days.sort_by(|a, b| b.date.cmp(&a.date));
    Ok(days)
}

fn read_day_from_dir(dir: &Path, date: &str) -> std::io::Result<ErrorLogRead> {
    validate_date(date)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;
    let path = dir.join(format!("{}.log", date));
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error),
    };
    Ok(ErrorLogRead {
        date: date.to_string(),
        line_count: count_entries(&content),
        content,
    })
}

fn prune_old_logs_from_dir(dir: &Path, today: &str) -> std::io::Result<()> {
    validate_date(today)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;
    if !dir.exists() {
        return Ok(());
    }
    let today = parse_date(today)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;
    let keep_after = today - time::Duration::days(RETENTION_DAYS - 1);

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("log") {
            continue;
        }
        let Some(date) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        let Ok(file_date) = parse_date(date) else {
            continue;
        };
        if file_date < keep_after {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn validate_date(value: &str) -> Result<(), String> {
    parse_date(value).map(|_| ())
}

fn parse_date(value: &str) -> Result<Date, String> {
    if value.len() != 10 {
        return Err("date must use YYYY-MM-DD".to_string());
    }
    let bytes = value.as_bytes();
    if bytes[4] != b'-' || bytes[7] != b'-' {
        return Err("date must use YYYY-MM-DD".to_string());
    }
    if !bytes
        .iter()
        .enumerate()
        .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
    {
        return Err("date must use YYYY-MM-DD".to_string());
    }
    let year: i32 = value[0..4]
        .parse()
        .map_err(|_| "invalid year".to_string())?;
    let month_num: u8 = value[5..7]
        .parse()
        .map_err(|_| "invalid month".to_string())?;
    let day: u8 = value[8..10]
        .parse()
        .map_err(|_| "invalid day".to_string())?;
    let month = Month::try_from(month_num).map_err(|_| "invalid month".to_string())?;
    Date::from_calendar_date(year, month, day).map_err(|_| "invalid date".to_string())
}

fn date_string(date: Date) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        date.year(),
        date.month() as u8,
        date.day()
    )
}

fn timestamp_string(now: OffsetDateTime) -> String {
    format!(
        "[{:04}-{:02}-{:02}][{:02}:{:02}:{:02}Z]",
        now.year(),
        now.month() as u8,
        now.day(),
        now.hour(),
        now.minute(),
        now.second()
    )
}

fn count_entries(content: &str) -> usize {
    content.lines().filter(|line| line.starts_with('[')).count()
}

fn non_empty_trimmed(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn sanitize_source(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
        .take(40)
        .collect::<String>()
}

fn sanitize_message(value: &str) -> String {
    crate::plugin_engine::host_api::redact_log_message(value).replace(['\r', '\n'], " ")
}

fn sanitize_stack(value: &str) -> String {
    crate::plugin_engine::host_api::redact_log_message(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_log_dir() -> std::path::PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("openusage-error-logs-test-{}", suffix));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn rejects_invalid_dates() {
        assert!(validate_date("2026-06-25").is_ok());
        assert!(validate_date("../2026-06-25").is_err());
        assert!(validate_date("2026-6-25").is_err());
        assert!(validate_date("2026-02-30").is_err());
    }

    #[test]
    fn writes_and_reads_daily_error_logs() {
        let dir = temp_log_dir();

        append_error_record(&dir, "2026-06-25", "[2026-06-25][app][ERROR] first").unwrap();
        append_error_record(
            &dir,
            "2026-06-25",
            "[2026-06-25][plugin:codex][ERROR] second",
        )
        .unwrap();

        let day = read_day_from_dir(&dir, "2026-06-25").unwrap();
        assert_eq!(day.date, "2026-06-25");
        assert_eq!(day.line_count, 2);
        assert!(day.content.contains("first"));
        assert!(day.content.contains("plugin:codex"));

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn lists_days_newest_first_with_counts() {
        let dir = temp_log_dir();
        append_error_record(&dir, "2026-06-24", "[2026-06-24][app][ERROR] older").unwrap();
        append_error_record(&dir, "2026-06-25", "[2026-06-25][app][ERROR] newer").unwrap();
        append_error_record(&dir, "2026-06-25", "[2026-06-25][app][ERROR] newer again").unwrap();

        let days = list_days_from_dir(&dir).unwrap();

        assert_eq!(days.len(), 2);
        assert_eq!(days[0].date, "2026-06-25");
        assert_eq!(days[0].count, 2);
        assert_eq!(days[1].date, "2026-06-24");
        assert_eq!(days[1].count, 1);

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn prunes_days_older_than_retention() {
        let dir = temp_log_dir();
        append_error_record(&dir, "2026-06-11", "[2026-06-11][app][ERROR] old").unwrap();
        append_error_record(&dir, "2026-06-12", "[2026-06-12][app][ERROR] keep").unwrap();
        append_error_record(&dir, "2026-06-25", "[2026-06-25][app][ERROR] newest").unwrap();

        prune_old_logs_from_dir(&dir, "2026-06-25").unwrap();

        assert!(!dir.join("2026-06-11.log").exists());
        assert!(dir.join("2026-06-12.log").exists());
        assert!(dir.join("2026-06-25.log").exists());

        fs::remove_dir_all(dir).unwrap();
    }
}
