use std::path::Path;

use anyhow::{Result, bail};
use chrono::{Days, Local, NaiveDate, TimeZone};

#[derive(Clone, Debug, Default)]
pub struct Query {
    pub terms: Vec<String>,
    pub folder: Option<String>,
    pub since: Option<i64>,
    pub until: Option<i64>,
}

impl Query {
    pub fn parse(input: &str) -> Result<Self> {
        if input.len() > 4096 {
            bail!("Search is limited to 4096 bytes");
        }
        let mut query = Self::default();
        let mut offset = 0;
        while let Some(word) = next_word(input, &mut offset) {
            if let Some(value) = word.strip_prefix("in:") {
                let mut folder = value.to_owned();
                if folder.starts_with('"') {
                    if !folder.ends_with('"') || folder.len() == 1 {
                        bail!("Unterminated quoted folder filter");
                    }
                    folder = folder[1..folder.len() - 1].into();
                }
                if !folder.is_empty() {
                    query.folder = Some(folder);
                }
                continue;
            }
            let date_token = ["date:", "after:", "before:"]
                .iter()
                .find_map(|p| word.strip_prefix(p).map(|v| (*p, v)));
            if let Some((kind, value)) = date_token {
                if value.len() < 10 && value.chars().all(|c| c.is_ascii_digit() || c == '-') {
                    continue;
                }
                let date = NaiveDate::parse_from_str(value, "%Y-%m-%d")
                    .map_err(|_| anyhow::anyhow!("Invalid date filter: use YYYY-MM-DD"))?;
                let start = day_start(date)?;
                match kind {
                    "before:" => {
                        query.until = Some(query.until.map_or(start, |old| old.min(start)))
                    }
                    "after:" => query.since = Some(query.since.map_or(start, |old| old.max(start))),
                    _ => {
                        let end = day_start(
                            date.checked_add_days(Days::new(1))
                                .ok_or_else(|| anyhow::anyhow!("Date is out of range"))?,
                        )?;
                        query.since = Some(query.since.map_or(start, |old| old.max(start)));
                        query.until = Some(query.until.map_or(end, |old| old.min(end)));
                    }
                }
                continue;
            }
            query.terms.push(word);
        }
        if query.terms.len() > 64 {
            bail!("Search is limited to 64 terms");
        }
        Ok(query)
    }

    pub fn matches_line(&self, text: &str) -> bool {
        let text = text.to_lowercase();
        self.terms
            .iter()
            .any(|term| text.contains(&term.to_lowercase()))
    }
}

pub fn terms_without_filters(input: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut offset = 0;
    while let Some(word) = next_word(input, &mut offset) {
        if word.strip_prefix("in:").is_some()
            || ["date:", "after:", "before:"]
                .iter()
                .any(|prefix| word.starts_with(prefix))
        {
            continue;
        }
        terms.push(word);
    }
    terms
}

pub(crate) fn normalize_folder(folder: &str) -> String {
    let path = Path::new(folder);
    if path.is_absolute() {
        std::fs::canonicalize(path)
            .unwrap_or_else(|_| path.to_owned())
            .to_string_lossy()
            .into_owned()
    } else {
        folder.to_owned()
    }
}

fn next_word(input: &str, offset: &mut usize) -> Option<String> {
    while *offset < input.len() {
        let character = input[*offset..].chars().next()?;
        if !character.is_whitespace() {
            break;
        }
        *offset += character.len_utf8();
    }
    if *offset >= input.len() {
        return None;
    }

    let start = *offset;
    if input[*offset..].starts_with("in:\"") {
        let folder_start = *offset + "in:\"".len();
        if let Some(relative_end) = input[folder_start..].find('"') {
            *offset = folder_start + relative_end + '"'.len_utf8();
            return Some(input[start..*offset].to_owned());
        }
    }

    while *offset < input.len() {
        let character = input[*offset..].chars().next()?;
        if character.is_whitespace() {
            break;
        }
        *offset += character.len_utf8();
    }
    Some(input[start..*offset].to_owned())
}

/// Formats a filesystem path as a quoted `in:` query filter.
///
/// The returned string is a complete query token. Paths must be valid UTF-8
/// and may not contain double quotes, because the query syntax has no escape
/// form for them.
pub fn folder_filter(path: &Path) -> Result<String> {
    let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_owned());
    let folder = path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Screenshot folder path is not valid UTF-8"))?;
    if folder.contains('"') {
        bail!("The quoted folder-filter syntax cannot represent this folder name");
    }
    Ok(format!("in:\"{folder}\""))
}

fn day_start(date: NaiveDate) -> Result<i64> {
    // Some time zones skip midnight: use the first representable instant of the day.
    for minutes in 0..=180 {
        let time = date
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| anyhow::anyhow!("Invalid date"))?
            + chrono::Duration::minutes(minutes);
        if let Some(value) = Local.from_local_datetime(&time).earliest() {
            return Ok(value.timestamp_millis());
        }
    }
    bail!("The selected local calendar date cannot be represented")
}

pub fn like_literal(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dates_and_quoted_folder_preserve_literal_terms() -> Result<()> {
        let q = Query::parse("error % in:\"/shots/my folder\" date:2026-10-07")?;
        assert_eq!(q.terms, ["error", "%"]);
        assert_eq!(q.folder.as_deref(), Some("/shots/my folder"));
        assert!(q.since < q.until);
        assert!(Query::parse("date:2026-02-31").is_err());
        assert!(Query::parse("date:2026-10").is_ok());
        Ok(())
    }

    #[test]
    fn quoted_folder_filters_preserve_whitespace() -> Result<()> {
        let path = Path::new("/shots/my  folder\twith\nspacing");
        let filter = folder_filter(path)?;
        let q = Query::parse(&format!("{filter} term"))?;
        assert_eq!(q.folder, path.to_str().map(str::to_owned));
        assert_eq!(q.terms, ["term"]);
        Ok(())
    }

    #[test]
    fn unterminated_folder_filters_are_rejected() {
        assert!(Query::parse("in:\"/shots/missing term").is_err());
    }

    #[test]
    fn manually_entered_absolute_folders_are_normalized() -> Result<()> {
        let root = tempfile::tempdir()?;
        let nested = root.path().join("nested");
        std::fs::create_dir(&nested)?;
        let alias = nested.join("..");
        let q = Query::parse(&folder_filter(&alias)?)?;
        assert_eq!(q.folder.as_deref(), Some(root.path().to_str().unwrap()));
        Ok(())
    }

    #[test]
    fn terms_without_filters_recovers_terms_from_malformed_queries() {
        assert_eq!(
            terms_without_filters("error in:\"/shots/unclosed date:2026-10"),
            ["error"]
        );
        assert_eq!(
            terms_without_filters("error in:\"/shots/my folder\" date:2026-10 term"),
            ["error", "term"]
        );
    }

    #[test]
    fn folder_filter_rejects_unrepresentable_paths() -> Result<()> {
        assert_eq!(
            folder_filter(Path::new("/shots/my folder"))?,
            "in:\"/shots/my folder\""
        );
        assert!(folder_filter(Path::new("/shots/my\"folder")).is_err());
        Ok(())
    }
}
