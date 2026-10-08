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
        let mut words = input.split_whitespace();
        while let Some(word) = words.next() {
            if let Some(value) = word.strip_prefix("in:") {
                let mut folder = value.to_owned();
                if folder.starts_with('"') {
                    while !folder.ends_with('"') || folder.len() == 1 {
                        let Some(next) = words.next() else {
                            return Ok(query);
                        };
                        folder.push(' ');
                        folder.push_str(next);
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
            query.terms.push(word.to_owned());
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
}
