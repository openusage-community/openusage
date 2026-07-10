use crate::plugin_engine::runtime::{MetricLine, PluginOutput};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
use time::format_description::well_known::Rfc3339;

const HISTORY_FILE_NAME: &str = "usage-history.json";
const HISTORY_VERSION: u32 = 1;
const HISTORY_RETENTION_DAYS: i64 = 365;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageHistoryRange {
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub row_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UsageHistoryFile {
    version: u32,
    snapshots: Vec<UsageHistorySnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UsageHistorySnapshot {
    provider_id: String,
    display_name: String,
    plan: Option<String>,
    lines: Vec<MetricLine>,
    fetched_at: String,
}

pub fn record_successful_output(
    app_data_dir: &Path,
    output: &PluginOutput,
    fetched_at: &str,
) -> Result<(), String> {
    if output_has_error(output) {
        return Ok(());
    }

    let mut snapshots = load_history(app_data_dir);
    snapshots.push(UsageHistorySnapshot {
        provider_id: output.provider_id.clone(),
        display_name: output.display_name.clone(),
        plan: output.plan.clone(),
        lines: output.lines.clone(),
        fetched_at: fetched_at.to_string(),
    });
    prune_history(&mut snapshots);
    save_history(app_data_dir, &snapshots)
}

pub fn list_range(app_data_dir: &Path) -> UsageHistoryRange {
    let snapshots = load_history(app_data_dir);
    let mut dates = snapshots
        .iter()
        .filter_map(|snapshot| fetched_date(&snapshot.fetched_at));
    let Some(first_date) = dates.next() else {
        return UsageHistoryRange {
            from_date: None,
            to_date: None,
            row_count: 0,
        };
    };

    let (min_date, max_date) = snapshots
        .iter()
        .filter_map(|snapshot| fetched_date(&snapshot.fetched_at))
        .fold((first_date.clone(), first_date), |(min, max), date| {
            (min.min(date.clone()), max.max(date))
        });

    UsageHistoryRange {
        from_date: Some(min_date),
        to_date: Some(max_date),
        row_count: snapshots.iter().map(|snapshot| snapshot.lines.len()).sum(),
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Csv,
    Xlsx,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportUsageHistoryResult {
    pub row_count: usize,
}

pub fn export_history(
    app_data_dir: &Path,
    format: ExportFormat,
    from_date: &str,
    to_date: &str,
    path: &Path,
) -> Result<ExportUsageHistoryResult, String> {
    let rows = export_rows(app_data_dir, from_date, to_date);
    match format {
        ExportFormat::Csv => write_csv(path, &rows)?,
        ExportFormat::Xlsx => write_xlsx(path, &rows)?,
    }
    Ok(ExportUsageHistoryResult {
        row_count: rows.len(),
    })
}

fn output_has_error(output: &PluginOutput) -> bool {
    output.lines.iter().any(|line| {
        matches!(
            line,
            MetricLine::Badge { label, .. } if label == "Error"
        )
    })
}

fn history_path(app_data_dir: &Path) -> std::path::PathBuf {
    app_data_dir.join(HISTORY_FILE_NAME)
}

fn load_history(app_data_dir: &Path) -> Vec<UsageHistorySnapshot> {
    let data = match std::fs::read_to_string(history_path(app_data_dir)) {
        Ok(data) => data,
        Err(_) => return Vec::new(),
    };
    match serde_json::from_str::<UsageHistoryFile>(&data) {
        Ok(file) if file.version == HISTORY_VERSION => file.snapshots,
        Ok(_) => {
            log::warn!("usage-history.json has unsupported version, starting empty");
            Vec::new()
        }
        Err(error) => {
            log::warn!(
                "failed to parse usage-history.json: {}, starting empty",
                error
            );
            Vec::new()
        }
    }
}

fn save_history(app_data_dir: &Path, snapshots: &[UsageHistorySnapshot]) -> Result<(), String> {
    std::fs::create_dir_all(app_data_dir)
        .map_err(|error| format!("failed to create app data dir: {}", error))?;
    let file = UsageHistoryFile {
        version: HISTORY_VERSION,
        snapshots: snapshots.to_vec(),
    };
    let json = serde_json::to_string(&file)
        .map_err(|error| format!("failed to serialize usage history: {}", error))?;
    let path = history_path(app_data_dir);
    let tmp_path = app_data_dir.join(".usage-history.json.tmp");
    std::fs::write(&tmp_path, json)
        .map_err(|error| format!("failed to write temp usage history: {}", error))?;
    std::fs::rename(&tmp_path, &path)
        .map_err(|error| format!("failed to rename usage history: {}", error))?;
    Ok(())
}

fn prune_history(snapshots: &mut Vec<UsageHistorySnapshot>) {
    let newest = snapshots
        .iter()
        .filter_map(|snapshot| parse_rfc3339(&snapshot.fetched_at))
        .max();
    let Some(newest) = newest else {
        return;
    };
    let cutoff = newest - time::Duration::days(HISTORY_RETENTION_DAYS);
    snapshots.retain(|snapshot| {
        parse_rfc3339(&snapshot.fetched_at)
            .map(|fetched_at| fetched_at > cutoff)
            .unwrap_or(false)
    });
}

fn parse_rfc3339(value: &str) -> Option<time::OffsetDateTime> {
    time::OffsetDateTime::parse(value, &Rfc3339).ok()
}

fn fetched_date(fetched_at: &str) -> Option<String> {
    let parsed = parse_rfc3339(fetched_at)?;
    let date = parsed.date();
    Some(format!(
        "{:04}-{:02}-{:02}",
        date.year(),
        date.month() as u8,
        date.day()
    ))
}

#[derive(Debug, Clone)]
struct ExportRow {
    fetched_at: String,
    provider_id: String,
    provider_name: String,
    plan: String,
    line_type: String,
    metric: String,
    used: Option<f64>,
    limit: Option<f64>,
    unit: String,
    value: String,
    reset_at: String,
}

fn export_rows(app_data_dir: &Path, from_date: &str, to_date: &str) -> Vec<ExportRow> {
    let mut rows = Vec::new();
    for snapshot in load_history(app_data_dir) {
        let Some(date) = fetched_date(&snapshot.fetched_at) else {
            continue;
        };
        if date.as_str() < from_date || date.as_str() > to_date {
            continue;
        }
        for line in snapshot.lines.clone() {
            rows.push(row_from_line(&snapshot, line));
        }
    }
    rows.sort_by(|a, b| {
        a.fetched_at
            .cmp(&b.fetched_at)
            .then(a.provider_id.cmp(&b.provider_id))
            .then(a.metric.cmp(&b.metric))
    });
    rows
}

fn row_from_line(snapshot: &UsageHistorySnapshot, line: MetricLine) -> ExportRow {
    match line {
        MetricLine::Progress {
            label,
            used,
            limit,
            format,
            resets_at,
            ..
        } => ExportRow {
            fetched_at: snapshot.fetched_at.clone(),
            provider_id: snapshot.provider_id.clone(),
            provider_name: snapshot.display_name.clone(),
            plan: snapshot.plan.clone().unwrap_or_default(),
            line_type: "progress".to_string(),
            metric: label,
            used: Some(used),
            limit: Some(limit),
            unit: progress_unit(format),
            value: String::new(),
            reset_at: resets_at.unwrap_or_default(),
        },
        MetricLine::Text { label, value, .. } => ExportRow {
            fetched_at: snapshot.fetched_at.clone(),
            provider_id: snapshot.provider_id.clone(),
            provider_name: snapshot.display_name.clone(),
            plan: snapshot.plan.clone().unwrap_or_default(),
            line_type: "text".to_string(),
            metric: label,
            used: None,
            limit: None,
            unit: String::new(),
            value,
            reset_at: String::new(),
        },
        MetricLine::Badge { label, text, .. } => ExportRow {
            fetched_at: snapshot.fetched_at.clone(),
            provider_id: snapshot.provider_id.clone(),
            provider_name: snapshot.display_name.clone(),
            plan: snapshot.plan.clone().unwrap_or_default(),
            line_type: "badge".to_string(),
            metric: label,
            used: None,
            limit: None,
            unit: String::new(),
            value: text,
            reset_at: String::new(),
        },
        MetricLine::BarChart {
            label,
            points,
            note,
            ..
        } => {
            let value = serde_json::json!({
                "points": points,
                "note": note,
            })
            .to_string();
            ExportRow {
                fetched_at: snapshot.fetched_at.clone(),
                provider_id: snapshot.provider_id.clone(),
                provider_name: snapshot.display_name.clone(),
                plan: snapshot.plan.clone().unwrap_or_default(),
                line_type: "barChart".to_string(),
                metric: label,
                used: None,
                limit: None,
                unit: String::new(),
                value,
                reset_at: String::new(),
            }
        }
    }
}

fn progress_unit(format: crate::plugin_engine::runtime::ProgressFormat) -> String {
    match format {
        crate::plugin_engine::runtime::ProgressFormat::Percent => "percent".to_string(),
        crate::plugin_engine::runtime::ProgressFormat::Dollars => "dollars".to_string(),
        crate::plugin_engine::runtime::ProgressFormat::Count { suffix } => suffix,
    }
}

fn write_csv(path: &Path, rows: &[ExportRow]) -> Result<(), String> {
    let mut csv = String::from(
        "fetchedAt,providerId,providerName,plan,lineType,metric,used,limit,unit,value,resetAt\n",
    );
    for row in rows {
        csv.push_str(&csv_line(row));
        csv.push('\n');
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create export dir: {}", error))?;
    }
    std::fs::write(path, csv).map_err(|error| format!("failed to write CSV export: {}", error))
}

fn csv_line(row: &ExportRow) -> String {
    [
        row.fetched_at.clone(),
        row.provider_id.clone(),
        row.provider_name.clone(),
        row.plan.clone(),
        row.line_type.clone(),
        row.metric.clone(),
        row.used.map(|value| value.to_string()).unwrap_or_default(),
        row.limit.map(|value| value.to_string()).unwrap_or_default(),
        row.unit.clone(),
        row.value.clone(),
        row.reset_at.clone(),
    ]
    .into_iter()
    .map(|value| escape_csv(&value))
    .collect::<Vec<_>>()
    .join(",")
}

fn escape_csv(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

#[derive(Debug, Clone)]
struct SummaryRow {
    provider_id: String,
    provider_name: String,
    metric: String,
    unit: String,
    count: usize,
    first_fetched_at: String,
    last_fetched_at: String,
    first: String,
    last: String,
    min: Option<f64>,
    max: Option<f64>,
}

fn summary_rows(rows: &[ExportRow]) -> Vec<SummaryRow> {
    let mut map: BTreeMap<(String, String, String), SummaryRow> = BTreeMap::new();
    for row in rows {
        let key = (
            row.provider_id.clone(),
            row.metric.clone(),
            row.unit.clone(),
        );
        let display_value = row
            .used
            .map(|value| value.to_string())
            .unwrap_or_else(|| row.value.clone());
        let entry = map.entry(key).or_insert_with(|| SummaryRow {
            provider_id: row.provider_id.clone(),
            provider_name: row.provider_name.clone(),
            metric: row.metric.clone(),
            unit: row.unit.clone(),
            count: 0,
            first_fetched_at: row.fetched_at.clone(),
            last_fetched_at: String::new(),
            first: display_value.clone(),
            last: String::new(),
            min: row.used,
            max: row.used,
        });
        entry.count += 1;
        entry.last_fetched_at = row.fetched_at.clone();
        entry.last = display_value;
        if let Some(used) = row.used {
            entry.min = Some(entry.min.map(|min| min.min(used)).unwrap_or(used));
            entry.max = Some(entry.max.map(|max| max.max(used)).unwrap_or(used));
        }
    }
    map.into_values().collect()
}

fn write_xlsx(path: &Path, rows: &[ExportRow]) -> Result<(), String> {
    let mut workbook = rust_xlsxwriter::Workbook::new();

    {
        let worksheet = workbook.add_worksheet();
        worksheet
            .set_name("Summary")
            .map_err(|error| format!("failed to name summary sheet: {}", error))?;
        let headers = [
            "providerId",
            "providerName",
            "metric",
            "unit",
            "count",
            "firstFetchedAt",
            "lastFetchedAt",
            "first",
            "last",
            "min",
            "max",
        ];
        write_xlsx_headers(worksheet, &headers)?;
        for (index, row) in summary_rows(rows).iter().enumerate() {
            let xlsx_row = (index + 1) as u32;
            worksheet
                .write_string(xlsx_row, 0, &row.provider_id)
                .map_err(xlsx_err)?;
            worksheet
                .write_string(xlsx_row, 1, &row.provider_name)
                .map_err(xlsx_err)?;
            worksheet
                .write_string(xlsx_row, 2, &row.metric)
                .map_err(xlsx_err)?;
            worksheet
                .write_string(xlsx_row, 3, &row.unit)
                .map_err(xlsx_err)?;
            worksheet
                .write_number(xlsx_row, 4, row.count as f64)
                .map_err(xlsx_err)?;
            worksheet
                .write_string(xlsx_row, 5, &row.first_fetched_at)
                .map_err(xlsx_err)?;
            worksheet
                .write_string(xlsx_row, 6, &row.last_fetched_at)
                .map_err(xlsx_err)?;
            worksheet
                .write_string(xlsx_row, 7, &row.first)
                .map_err(xlsx_err)?;
            worksheet
                .write_string(xlsx_row, 8, &row.last)
                .map_err(xlsx_err)?;
            if let Some(value) = row.min {
                worksheet
                    .write_number(xlsx_row, 9, value)
                    .map_err(xlsx_err)?;
            }
            if let Some(value) = row.max {
                worksheet
                    .write_number(xlsx_row, 10, value)
                    .map_err(xlsx_err)?;
            }
        }
    }

    {
        let worksheet = workbook.add_worksheet();
        worksheet
            .set_name("Snapshots")
            .map_err(|error| format!("failed to name snapshots sheet: {}", error))?;
        let headers = [
            "fetchedAt",
            "providerId",
            "providerName",
            "plan",
            "lineType",
            "metric",
            "used",
            "limit",
            "unit",
            "value",
            "resetAt",
        ];
        write_xlsx_headers(worksheet, &headers)?;
        for (index, row) in rows.iter().enumerate() {
            let xlsx_row = (index + 1) as u32;
            worksheet
                .write_string(xlsx_row, 0, &row.fetched_at)
                .map_err(xlsx_err)?;
            worksheet
                .write_string(xlsx_row, 1, &row.provider_id)
                .map_err(xlsx_err)?;
            worksheet
                .write_string(xlsx_row, 2, &row.provider_name)
                .map_err(xlsx_err)?;
            worksheet
                .write_string(xlsx_row, 3, &row.plan)
                .map_err(xlsx_err)?;
            worksheet
                .write_string(xlsx_row, 4, &row.line_type)
                .map_err(xlsx_err)?;
            worksheet
                .write_string(xlsx_row, 5, &row.metric)
                .map_err(xlsx_err)?;
            if let Some(value) = row.used {
                worksheet
                    .write_number(xlsx_row, 6, value)
                    .map_err(xlsx_err)?;
            }
            if let Some(value) = row.limit {
                worksheet
                    .write_number(xlsx_row, 7, value)
                    .map_err(xlsx_err)?;
            }
            worksheet
                .write_string(xlsx_row, 8, &row.unit)
                .map_err(xlsx_err)?;
            worksheet
                .write_string(xlsx_row, 9, &row.value)
                .map_err(xlsx_err)?;
            worksheet
                .write_string(xlsx_row, 10, &row.reset_at)
                .map_err(xlsx_err)?;
        }
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create export dir: {}", error))?;
    }
    workbook
        .save(path)
        .map_err(|error| format!("failed to write XLSX export: {}", error))
}

fn write_xlsx_headers(
    worksheet: &mut rust_xlsxwriter::Worksheet,
    headers: &[&str],
) -> Result<(), String> {
    for (index, header) in headers.iter().enumerate() {
        worksheet
            .write_string(0, index as u16, *header)
            .map_err(xlsx_err)?;
    }
    Ok(())
}

fn xlsx_err(error: rust_xlsxwriter::XlsxError) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin_engine::runtime::ProgressFormat;

    fn temp_dir(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "openusage-history-test-{}-{}",
            label,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn make_output(id: &str, used: f64) -> PluginOutput {
        PluginOutput {
            provider_id: id.to_string(),
            display_name: format!("Provider {}", id),
            plan: Some("Pro".to_string()),
            warning: None,
            lines: vec![
                MetricLine::Progress {
                    label: "Session".to_string(),
                    used,
                    limit: 100.0,
                    format: ProgressFormat::Percent,
                    resets_at: Some("2026-06-17T00:00:00Z".to_string()),
                    period_duration_ms: Some(86_400_000),
                    color: None,
                },
                MetricLine::Text {
                    label: "Today".to_string(),
                    value: "$12.34 · 56K tokens".to_string(),
                    color: None,
                    subtitle: None,
                    model_breakdown: None,
                    status_dot: None,
                    expiry_tooltip: None,
                },
            ],
            icon_url: String::new(),
        }
    }

    fn make_error_output(id: &str) -> PluginOutput {
        PluginOutput {
            provider_id: id.to_string(),
            display_name: format!("Provider {}", id),
            plan: None,
            warning: None,
            lines: vec![MetricLine::Badge {
                label: "Error".to_string(),
                text: "Failed".to_string(),
                color: None,
                subtitle: None,
            }],
            icon_url: String::new(),
        }
    }

    #[test]
    fn history_range_returns_empty_for_missing_file() {
        let dir = temp_dir("missing");

        let range = list_range(&dir);

        assert_eq!(range.from_date, None);
        assert_eq!(range.to_date, None);
        assert_eq!(range.row_count, 0);
    }

    #[test]
    fn record_successful_output_skips_error_snapshots() {
        let dir = temp_dir("skip-error");
        std::fs::create_dir_all(&dir).unwrap();

        record_successful_output(&dir, &make_error_output("claude"), "2026-06-16T10:00:00Z")
            .unwrap();

        assert_eq!(list_range(&dir).row_count, 0);
        assert!(!dir.join(HISTORY_FILE_NAME).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn records_history_and_lists_available_range() {
        let dir = temp_dir("range");
        std::fs::create_dir_all(&dir).unwrap();

        record_successful_output(&dir, &make_output("claude", 10.0), "2026-06-14T23:59:00Z")
            .unwrap();
        record_successful_output(&dir, &make_output("codex", 20.0), "2026-06-16T00:01:00Z")
            .unwrap();

        let range = list_range(&dir);

        assert_eq!(range.from_date, Some("2026-06-14".to_string()));
        assert_eq!(range.to_date, Some("2026-06-16".to_string()));
        assert_eq!(range.row_count, 4);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn csv_export_filters_dates_inclusively_and_escapes_values() {
        let dir = temp_dir("csv");
        std::fs::create_dir_all(&dir).unwrap();
        let output = PluginOutput {
            provider_id: "claude".to_string(),
            display_name: "Claude, Inc".to_string(),
            plan: Some("Team \"Pro\"".to_string()),
            warning: None,
            lines: vec![MetricLine::Text {
                label: "Today".to_string(),
                value: "line one\nline two".to_string(),
                color: None,
                subtitle: None,
                model_breakdown: None,
                status_dot: None,
                expiry_tooltip: None,
            }],
            icon_url: String::new(),
        };

        record_successful_output(&dir, &make_output("codex", 5.0), "2026-06-13T23:59:59Z").unwrap();
        record_successful_output(&dir, &output, "2026-06-14T00:00:00Z").unwrap();
        record_successful_output(&dir, &make_output("gemini", 7.0), "2026-06-16T00:00:00Z")
            .unwrap();

        let path = dir.join("usage.csv");
        let result =
            export_history(&dir, ExportFormat::Csv, "2026-06-14", "2026-06-14", &path).unwrap();
        let csv = std::fs::read_to_string(&path).unwrap();

        assert_eq!(result.row_count, 1);
        assert!(csv.starts_with(
            "fetchedAt,providerId,providerName,plan,lineType,metric,used,limit,unit,value,resetAt\n"
        ));
        assert!(csv.contains("\"Claude, Inc\""));
        assert!(csv.contains("\"Team \"\"Pro\"\"\""));
        assert!(csv.contains("\"line one\nline two\""));
        assert!(!csv.contains("codex"));
        assert!(!csv.contains("gemini"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn xlsx_export_creates_workbook_with_snapshot_and_summary_sheets() {
        use std::io::Read;

        let dir = temp_dir("xlsx");
        std::fs::create_dir_all(&dir).unwrap();
        record_successful_output(&dir, &make_output("claude", 10.0), "2026-06-14T10:00:00Z")
            .unwrap();
        record_successful_output(&dir, &make_output("claude", 30.0), "2026-06-14T12:00:00Z")
            .unwrap();

        let path = dir.join("usage.xlsx");
        let result =
            export_history(&dir, ExportFormat::Xlsx, "2026-06-14", "2026-06-14", &path).unwrap();
        let file = std::fs::File::open(&path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let mut workbook_xml = String::new();
        archive
            .by_name("xl/workbook.xml")
            .unwrap()
            .read_to_string(&mut workbook_xml)
            .unwrap();

        assert_eq!(result.row_count, 4);
        assert!(workbook_xml.contains(r#"name="Summary""#));
        assert!(workbook_xml.contains(r#"name="Snapshots""#));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn summary_rows_include_snapshot_timestamps_to_disambiguate_metric_periods() {
        let dir = temp_dir("summary-timestamps");
        std::fs::create_dir_all(&dir).unwrap();
        record_successful_output(&dir, &make_output("codex", 10.0), "2026-06-16T08:00:00Z")
            .unwrap();
        record_successful_output(&dir, &make_output("codex", 30.0), "2026-06-16T09:00:00Z")
            .unwrap();

        let rows = export_rows(&dir, "2026-06-16", "2026-06-16");
        let summary = summary_rows(&rows);
        let today = summary
            .iter()
            .find(|row| row.metric == "Today")
            .expect("today metric summary");

        assert_eq!(today.count, 2);
        assert_eq!(today.first_fetched_at, "2026-06-16T08:00:00Z");
        assert_eq!(today.last_fetched_at, "2026-06-16T09:00:00Z");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn retention_keeps_only_last_365_days() {
        let dir = temp_dir("retention");
        std::fs::create_dir_all(&dir).unwrap();

        record_successful_output(&dir, &make_output("old", 1.0), "2025-06-15T00:00:00Z").unwrap();
        record_successful_output(&dir, &make_output("new", 2.0), "2026-06-15T00:00:00Z").unwrap();

        let path = dir.join("usage.csv");
        export_history(&dir, ExportFormat::Csv, "2025-06-15", "2026-06-15", &path).unwrap();
        let csv = std::fs::read_to_string(&path).unwrap();

        assert!(!csv.contains(",old,"));
        assert!(csv.contains(",new,"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
