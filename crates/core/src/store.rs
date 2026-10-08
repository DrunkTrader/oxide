use std::{
    fs,
    path::Path,
    time::{Duration, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params, types::Value};

use crate::{
    Line, Note, NoteSummary, Screenshot, media, now,
    query::{Query, like_literal},
};

pub struct Store {
    connection: Connection,
}

const SCHEMA: &str = "
CREATE TABLE notes(id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT NOT NULL, body TEXT NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, revision INTEGER NOT NULL DEFAULT 0, deleted_at INTEGER);
CREATE TABLE screenshots(id INTEGER PRIMARY KEY AUTOINCREMENT, path TEXT NOT NULL UNIQUE, managed INTEGER NOT NULL, retained INTEGER NOT NULL DEFAULT 0, captured_at INTEGER NOT NULL, time_source TEXT NOT NULL, mtime_ns TEXT NOT NULL, file_size INTEGER NOT NULL, width INTEGER NOT NULL, height INTEGER NOT NULL, digest TEXT NOT NULL, available INTEGER NOT NULL DEFAULT 1, ocr_state TEXT NOT NULL DEFAULT 'pending', ocr_error TEXT, ocr_version TEXT, updated_at INTEGER NOT NULL);
CREATE TABLE note_screenshots(note_id INTEGER NOT NULL REFERENCES notes(id) ON DELETE CASCADE, screenshot_id INTEGER NOT NULL REFERENCES screenshots(id), position INTEGER NOT NULL, attached_at INTEGER NOT NULL, PRIMARY KEY(note_id, screenshot_id));
CREATE TABLE ocr_lines(screenshot_id INTEGER NOT NULL REFERENCES screenshots(id) ON DELETE CASCADE, position INTEGER NOT NULL, text TEXT NOT NULL, x REAL NOT NULL, y REAL NOT NULL, w REAL NOT NULL, h REAL NOT NULL, confidence REAL, PRIMARY KEY(screenshot_id, position));
CREATE VIRTUAL TABLE notes_fts USING fts5(text, tokenize='trigram');
CREATE VIRTUAL TABLE screenshots_fts USING fts5(text, tokenize='trigram');
CREATE TABLE changes(id INTEGER PRIMARY KEY CHECK(id=1), revision INTEGER NOT NULL);
INSERT INTO changes VALUES(1,0);
CREATE INDEX notes_date ON notes(updated_at);
CREATE INDEX screenshots_date ON screenshots(captured_at);
CREATE INDEX screenshots_pending ON screenshots(ocr_state, available, captured_at);
CREATE INDEX attachments_screenshot ON note_screenshots(screenshot_id);
PRAGMA user_version=3;
";

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut connection = Connection::open(path).context("Cannot open Oxide database")?;
        connection.busy_timeout(Duration::from_millis(1500))?;
        connection.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;",
        )?;
        let existing_version: i64 =
            connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if matches!(existing_version, 1 | 2) {
            let parent = path.parent().context("Database has no parent")?;
            let backup = parent.join(format!("oxide-before-schema-3-{}.db", now()));
            backup_database(&connection, &backup)
                .context("Cannot back up database before migration")?;
        }
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let version: i64 = tx.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        match version {
            0 => {
                let tables: i64 = tx.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'", [], |r| r.get(0))?;
                if tables != 0 {
                    bail!(
                        "Unversioned existing database refused; restore a backup instead of resetting user data"
                    );
                }
                tx.execute_batch(SCHEMA)?;
            }
            1 | 2 => {
                if version == 1 {
                    tx.execute_batch(
                        "ALTER TABLE screenshots ADD COLUMN retained INTEGER NOT NULL DEFAULT 0;",
                    )?;
                }
                tx.execute_batch("ALTER TABLE screenshots ADD COLUMN time_source TEXT NOT NULL DEFAULT 'legacy'; UPDATE screenshots SET time_source='modified' WHERE managed=0; PRAGMA user_version=3;")?;
            }
            3 => {}
            _ => bail!(
                "Unsupported database schema {version}; user data was not reset. Use a compatible Oxide build or restore a backup"
            ),
        }
        tx.commit()?;
        Ok(Self { connection })
    }

    pub fn revision(&self) -> Result<i64> {
        Ok(self
            .connection
            .query_row("SELECT revision FROM changes WHERE id=1", [], |r| r.get(0))?)
    }

    pub fn new_note(&mut self) -> Result<Note> {
        let time = now();
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT INTO notes(title,body,created_at,updated_at) VALUES('Untitled note','',?1,?1)",
            [time],
        )?;
        let id = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO notes_fts(rowid,text) VALUES(?1,'Untitled note')",
            [id],
        )?;
        changed(&tx)?;
        tx.commit()?;
        self.note(id)?.context("New note could not be read")
    }

    pub fn note(&self, id: i64) -> Result<Option<Note>> {
        Ok(self
            .connection
            .query_row(
                "SELECT id,title,body,updated_at,revision,deleted_at FROM notes WHERE id=?1",
                [id],
                |r| {
                    Ok(Note {
                        id: r.get(0)?,
                        title: r.get(1)?,
                        body: r.get(2)?,
                        updated_at: r.get(3)?,
                        revision: r.get(4)?,
                        deleted_at: r.get(5)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn save_note(&mut self, note: &Note) -> Result<Note> {
        if note.title.len() > 8192 || note.body.len() > 4 * 1024 * 1024 {
            bail!(
                "Note exceeds the 8 KiB title or 4 MiB body limit; export your buffer before splitting the note"
            );
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if tx.execute("UPDATE notes SET title=?1,body=?2,updated_at=?3,revision=revision+1 WHERE id=?4 AND revision=?5 AND deleted_at IS NULL", params![note.title, note.body, now(), note.id, note.revision])? != 1 {
            bail!("Note changed elsewhere or was removed; your editing buffer was retained");
        }
        tx.execute("DELETE FROM notes_fts WHERE rowid=?1", [note.id])?;
        tx.execute(
            "INSERT INTO notes_fts(rowid,text) VALUES(?1,?2)",
            params![note.id, format!("{}\n{}", note.title, note.body)],
        )?;
        changed(&tx)?;
        tx.commit()?;
        self.note(note.id)?.context("Saved note could not be read")
    }

    pub fn set_note_trashed(&mut self, id: i64, trash: bool) -> Result<()> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if tx.execute(
            "UPDATE notes SET deleted_at=?1,revision=revision+1 WHERE id=?2",
            params![trash.then(now), id],
        )? != 1
        {
            bail!("Note no longer exists");
        }
        changed(&tx)?;
        tx.commit()?;
        Ok(())
    }

    pub fn duplicate_note(&mut self, id: i64) -> Result<Note> {
        let original = self.note(id)?.context("Note no longer exists")?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let title = format!("{} (copy)", original.title);
        tx.execute(
            "INSERT INTO notes(title,body,created_at,updated_at) VALUES(?1,?2,?3,?3)",
            params![title, original.body, now()],
        )?;
        let new = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO notes_fts(rowid,text) VALUES(?1,?2)",
            params![new, format!("{title}\n{}", original.body)],
        )?;
        tx.execute("INSERT INTO note_screenshots SELECT ?1,screenshot_id,position,?2 FROM note_screenshots WHERE note_id=?3",params![new,now(),id])?;
        changed(&tx)?;
        tx.commit()?;
        self.note(new)?.context("Copied note could not be read")
    }

    pub fn notes(
        &self,
        query: &Query,
        trash: bool,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<NoteSummary>> {
        if query.folder.is_some() {
            return Ok(Vec::new());
        }
        let (condition, mut values, order, join) =
            search_clause(query, "n.updated_at", "notes_fts", "n.id", None);
        values.push((limit as i64).into());
        values.push((offset as i64).into());
        let sql = format!(
            "SELECT n.id,n.title,n.updated_at,substr(n.body,1,120) FROM notes n {join} WHERE n.deleted_at IS {}NULL {condition} ORDER BY {order} n.updated_at DESC,n.id DESC LIMIT ? OFFSET ?",
            if trash { "NOT " } else { "" }
        );
        let mut stmt = self.connection.prepare(&sql)?;
        Ok(stmt
            .query_map(rusqlite::params_from_iter(values), |r| {
                Ok(NoteSummary {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    updated_at: r.get(2)?,
                    excerpt: r.get(3)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?)
    }

    pub fn register_image(
        &mut self,
        path: &Path,
        managed: bool,
        captured_at: Option<i64>,
    ) -> Result<Screenshot> {
        if !media::supported(path) {
            bail!("Supported image formats are PNG, JPEG, and WebP");
        }
        let path = fs::canonicalize(path).context("Image source is missing or unavailable")?;
        let text = path.to_str().context("Image path is not valid Unicode")?;
        let metadata = fs::metadata(&path)?;
        let mtime_ns = match metadata.modified()?.duration_since(UNIX_EPOCH) {
            Ok(delta) => i128::try_from(delta.as_nanos())?,
            Err(error) => -i128::try_from(error.duration().as_nanos())?,
        };
        let time = captured_at.unwrap_or(i64::try_from(mtime_ns.div_euclid(1_000_000))?);
        let mtime_ns = mtime_ns.to_string();
        let size = i64::try_from(metadata.len())?;
        let old = self
            .connection
            .query_row(
                "SELECT id,mtime_ns,file_size FROM screenshots WHERE path=?1",
                [text],
                |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()?;
        if let Some((id, old_time, old_size)) = &old
            && old_time == &mtime_ns
            && *old_size == size
        {
            let shot = self
                .screenshot(*id)?
                .context("Image metadata could not be read")?;
            if shot.available
                && captured_at.is_none()
                && (!managed || shot.managed)
                && matches!(
                    shot.ocr_state.as_str(),
                    "ready" | "no_text" | "failed" | "invalid"
                )
            {
                return Ok(shot);
            }
        }
        let (width, height, image_error) = match media::load_image(&path) {
            Ok(image) => (image.width(), image.height(), None),
            Err(error) => (0, 0, Some(format!("{error:#}"))),
        };
        let digest = if image_error.is_none() {
            media::fingerprint(&path)?
        } else {
            String::new()
        };
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        // A move with identical bytes reuses identity only when the previous original is gone.
        let mut moved_id = None;
        if old.is_none() && image_error.is_none() {
            let mut stmt = tx.prepare("SELECT id,path FROM screenshots WHERE digest=?1")?;
            let candidates = stmt
                .query_map(params![digest], |r| {
                    Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            let absent: Vec<_> = candidates
                .into_iter()
                .filter(|(_, p)| {
                    let path = Path::new(p);
                    path.try_exists().is_ok_and(|exists| !exists)
                        && path.parent().is_some_and(Path::is_dir)
                })
                .collect();
            if absent.len() == 1 {
                moved_id = Some(absent[0].0);
            }
        }
        let id = if let Some((id, _, _)) = old {
            id
        } else if let Some(id) = moved_id {
            tx.execute(
                "UPDATE screenshots SET path=?1 WHERE id=?2",
                params![text, id],
            )?;
            id
        } else {
            tx.execute("INSERT INTO screenshots(path,managed,captured_at,mtime_ns,file_size,width,height,digest,updated_at,time_source) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",params![text,managed,time,mtime_ns,size,width,height,digest,now(),if captured_at.is_some(){"captured"}else{"modified"}])?;
            tx.last_insert_rowid()
        };
        tx.execute("UPDATE screenshots SET mtime_ns=?1,file_size=?2,width=?3,height=?4,ocr_state=CASE WHEN ?9 IS NOT NULL THEN 'invalid' WHEN digest=?5 AND ocr_state IN ('ready','no_text') THEN ocr_state ELSE 'pending' END,ocr_error=?9,digest=?5,available=1,updated_at=max(updated_at+1,?6),managed=max(managed,?8),captured_at=coalesce(?10,captured_at),time_source=CASE WHEN ?10 IS NOT NULL THEN 'captured' ELSE time_source END WHERE id=?7",params![mtime_ns,size,width,height,digest,now(),id,managed,image_error,captured_at])?;
        if image_error.is_some() {
            tx.execute("DELETE FROM screenshots_fts WHERE rowid=?1", [id])?;
            tx.execute("DELETE FROM ocr_lines WHERE screenshot_id=?1", [id])?;
        }
        changed(&tx)?;
        tx.commit()?;
        self.screenshot(id)?
            .context("Registered image could not be read")
    }

    pub fn screenshot(&self, id: i64) -> Result<Option<Screenshot>> {
        Ok(self
            .connection
            .query_row(&format!("{SHOT_SELECT} WHERE s.id=?1"), [id], read_shot)
            .optional()?)
    }

    pub fn is_original(&self, path: &Path) -> Result<bool> {
        let path = fs::canonicalize(path)?;
        Ok(self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM screenshots WHERE path=?1)",
            [path.to_str().context("Image path is not Unicode")?],
            |row| row.get(0),
        )?)
    }

    pub fn register_import(&mut self, path: &Path) -> Result<Screenshot> {
        let shot = self.register_image(path, false, None)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute("UPDATE screenshots SET retained=1,ocr_state=CASE WHEN ocr_state='unwatched' THEN 'pending' ELSE ocr_state END,updated_at=updated_at+1 WHERE id=?1",[shot.id])?;
        changed(&tx)?;
        tx.commit()?;
        self.screenshot(shot.id)?
            .context("Imported image could not be read")
    }

    pub fn rename_source(&mut self, from: &Path, to: &Path) -> Result<bool> {
        let from = normalize_source_path(from);
        let destination = normalize_source_path(to);
        let from_text = from.to_str().context("Source path is not Unicode")?;
        let prefix = format!(
            "{}%",
            like_literal(&format!(
                "{}{}",
                from_text.trim_end_matches(['/', '\\']),
                std::path::MAIN_SEPARATOR
            ))
        );
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let rows = tx
            .prepare("SELECT id,path FROM screenshots WHERE path LIKE ?1 ESCAPE '\\'")?
            .query_map([prefix], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        if !rows.is_empty() {
            let mut count = 0;
            for (id, path) in rows {
                if let Ok(relative) = Path::new(&path).strip_prefix(&from) {
                    let target = destination.join(relative);
                    tx.execute("UPDATE screenshots SET path=?1,updated_at=max(updated_at+1,?2) WHERE id=?3",params![target.to_str().context("Destination path is not Unicode")?,now(),id]).context("Directory rename conflicts with another screenshot identity; all original relationships were retained")?;
                    count += 1;
                }
            }
            if count > 0 {
                changed(&tx)?;
            }
            tx.commit()?;
            return Ok(count > 0);
        }
        let from = from_text;
        let to = destination
            .to_str()
            .context("Destination path is not Unicode")?;
        let id = tx
            .query_row("SELECT id FROM screenshots WHERE path=?1", [from], |r| {
                r.get::<_, i64>(0)
            })
            .optional()?;
        let Some(id) = id else {
            return Ok(false);
        };
        let occupied = tx
            .query_row("SELECT id FROM screenshots WHERE path=?1", [to], |r| {
                r.get::<_, i64>(0)
            })
            .optional()?;
        if occupied.is_some_and(|other| other != id) {
            bail!(
                "Rename destination already has a different screenshot identity; both attachment records were retained"
            );
        }
        tx.execute(
            "UPDATE screenshots SET path=?1,updated_at=max(updated_at+1,?2) WHERE id=?3",
            params![to, now(), id],
        )?;
        changed(&tx)?;
        tx.commit()?;
        Ok(true)
    }

    pub fn screenshots(
        &self,
        query: &Query,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Screenshot>> {
        let (condition, mut values, order, join) = search_clause(
            query,
            "s.captured_at",
            "screenshots_fts",
            "s.id",
            Some("s.path"),
        );
        values.push((limit as i64).into());
        values.push((offset as i64).into());
        let sql = format!(
            "{SHOT_SELECT} {join} WHERE s.ocr_state NOT IN ('trashed','unwatched') {condition} ORDER BY {order} s.captured_at DESC,s.id DESC LIMIT ? OFFSET ?"
        );
        let mut stmt = self.connection.prepare(&sql)?;
        Ok(stmt
            .query_map(rusqlite::params_from_iter(values), read_shot)?
            .collect::<rusqlite::Result<_>>()?)
    }

    pub fn pending(&self) -> Result<Option<Screenshot>> {
        Ok(self.connection.query_row(&format!("{SHOT_SELECT} WHERE available=1 AND ocr_state='pending' ORDER BY managed DESC,captured_at DESC,id DESC LIMIT 1"),[],read_shot).optional()?)
    }

    pub fn attachments(&self, note: i64) -> Result<Vec<Screenshot>> {
        let mut stmt = self.connection.prepare(&format!("{SHOT_SELECT} JOIN note_screenshots a ON a.screenshot_id=s.id WHERE a.note_id=?1 ORDER BY a.position,a.screenshot_id"))?;
        Ok(stmt
            .query_map([note], read_shot)?
            .collect::<rusqlite::Result<_>>()?)
    }

    pub fn linked_notes(&self, screenshot: i64) -> Result<Vec<NoteSummary>> {
        let mut stmt = self.connection.prepare("SELECT n.id,n.title,n.updated_at,substr(n.body,1,120) FROM notes n JOIN note_screenshots a ON a.note_id=n.id WHERE a.screenshot_id=?1 AND n.deleted_at IS NULL ORDER BY n.updated_at DESC")?;
        Ok(stmt
            .query_map([screenshot], |r| {
                Ok(NoteSummary {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    updated_at: r.get(2)?,
                    excerpt: r.get(3)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?)
    }

    pub fn attach(&mut self, note: i64, shot: i64) -> Result<()> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let active: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM notes WHERE id=?1 AND deleted_at IS NULL)",
            [note],
            |r| r.get(0),
        )?;
        if !active {
            bail!("Attachment destination was removed; the screenshot remains in the library");
        }
        tx.execute("INSERT OR IGNORE INTO note_screenshots VALUES(?1,?2,(SELECT coalesce(max(position),-1)+1 FROM note_screenshots WHERE note_id=?1),?3)",params![note,shot,now()])?;
        changed(&tx)?;
        tx.commit()?;
        Ok(())
    }

    pub fn detach(&mut self, note: i64, shot: i64) -> Result<()> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "DELETE FROM note_screenshots WHERE note_id=?1 AND screenshot_id=?2",
            params![note, shot],
        )?;
        changed(&tx)?;
        tx.commit()?;
        Ok(())
    }

    pub fn lines(&self, id: i64) -> Result<Vec<Line>> {
        let mut stmt = self.connection.prepare("SELECT text,x,y,w,h,confidence FROM ocr_lines WHERE screenshot_id=?1 ORDER BY position")?;
        Ok(stmt
            .query_map([id], |r| {
                Ok(Line {
                    text: r.get(0)?,
                    x: r.get(1)?,
                    y: r.get(2)?,
                    w: r.get(3)?,
                    h: r.get(4)?,
                    confidence: r.get(5)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?)
    }

    pub fn set_ocr(&mut self, shot: &Screenshot, lines: &[Line], version: &str) -> Result<bool> {
        if lines.iter().any(|line| !line.valid()) {
            bail!("OCR produced invalid line geometry/confidence");
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if tx.execute("UPDATE screenshots SET ocr_state=?1,ocr_error=NULL,ocr_version=?2,updated_at=max(updated_at+1,?3) WHERE id=?4 AND digest=?5 AND available=1 AND ocr_state NOT IN ('trashed','unwatched')",params![if lines.is_empty(){"no_text"}else{"ready"},version,now(),shot.id,shot.digest])? == 0 { return Ok(false); }
        tx.execute("DELETE FROM ocr_lines WHERE screenshot_id=?1", [shot.id])?;
        for (position, line) in lines.iter().enumerate() {
            tx.execute(
                "INSERT INTO ocr_lines VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                params![
                    shot.id,
                    position as i64,
                    line.text,
                    line.x,
                    line.y,
                    line.w,
                    line.h,
                    line.confidence
                ],
            )?;
        }
        tx.execute("DELETE FROM screenshots_fts WHERE rowid=?1", [shot.id])?;
        tx.execute(
            "INSERT INTO screenshots_fts(rowid,text) VALUES(?1,?2)",
            params![
                shot.id,
                lines
                    .iter()
                    .map(|l| l.text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            ],
        )?;
        changed(&tx)?;
        tx.commit()?;
        Ok(true)
    }

    pub fn ocr_failed(&mut self, id: i64, message: &str) -> Result<()> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "UPDATE screenshots SET ocr_state='failed',ocr_error=?1,updated_at=max(updated_at+1,?2) WHERE id=?3 AND ocr_state NOT IN ('trashed','unwatched')",
            params![message, now(), id],
        )?;
        changed(&tx)?;
        tx.commit()?;
        Ok(())
    }

    pub fn reindex(&mut self, id: Option<i64>) -> Result<()> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute("UPDATE screenshots SET ocr_state='pending',ocr_error=NULL,updated_at=max(updated_at+1,?2) WHERE available=1 AND ocr_state NOT IN ('trashed','unwatched') AND (?1 IS NULL OR id=?1)",params![id,now()])?;
        changed(&tx)?;
        tx.commit()?;
        Ok(())
    }

    pub fn reindex_outdated(&mut self, version: &str) -> Result<()> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if tx.execute("UPDATE screenshots SET ocr_state='pending',ocr_error=NULL,updated_at=updated_at+1 WHERE available=1 AND ocr_state IN ('ready','no_text') AND (ocr_version IS NULL OR ocr_version!=?1)",[version])?>0{changed(&tx)?;}
        tx.commit()?;
        Ok(())
    }

    pub fn set_available(&mut self, id: i64, available: bool) -> Result<()> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let count = tx.execute(
            "UPDATE screenshots SET available=?1,updated_at=max(updated_at+1,?2) WHERE id=?3 AND available!=?1",
            params![available, now(), id],
        )?;
        if count > 0 {
            changed(&tx)?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn source_rows(&self, after: i64) -> Result<Vec<(i64, String, bool, bool)>> {
        let mut stmt = self.connection.prepare(
            "SELECT id,path,managed,retained FROM screenshots WHERE id>?1 ORDER BY id LIMIT 256",
        )?;
        Ok(stmt
            .query_map([after], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })?
            .collect::<rusqlite::Result<_>>()?)
    }

    pub fn trash_image(&mut self, id: i64) -> Result<()> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "UPDATE screenshots SET available=0,ocr_state='trashed',updated_at=max(updated_at+1,?1) WHERE id=?2",
            params![now(), id],
        )?;
        tx.execute("DELETE FROM screenshots_fts WHERE rowid=?1", [id])?;
        changed(&tx)?;
        tx.commit()?;
        Ok(())
    }

    pub fn pending_count(&self) -> Result<usize> {
        let count: i64 = self.connection.query_row(
            "SELECT count(*) FROM screenshots WHERE available=1 AND ocr_state='pending'",
            [],
            |row| row.get(0),
        )?;
        Ok(count.try_into()?)
    }

    pub fn clear_derived(&mut self) -> Result<()> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute_batch("DELETE FROM ocr_lines; DELETE FROM screenshots_fts; DELETE FROM notes_fts; INSERT INTO notes_fts(rowid,text) SELECT id,title || char(10) || body FROM notes; UPDATE screenshots SET ocr_state='pending',ocr_error=NULL,updated_at=updated_at+1 WHERE ocr_state NOT IN ('trashed','unwatched');")?;
        changed(&tx)?;
        tx.commit()?;
        Ok(())
    }

    pub fn forget_ocr(&mut self, id: i64) -> Result<()> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (state, protected): (String, bool) = tx.query_row(
            "SELECT ocr_state,(managed OR retained) FROM screenshots WHERE id=?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        if protected || state == "unwatched" || state == "trashed" {
            return Ok(());
        }
        tx.execute("DELETE FROM screenshots_fts WHERE rowid=?1", [id])?;
        tx.execute("DELETE FROM ocr_lines WHERE screenshot_id=?1", [id])?;
        tx.execute(
            "UPDATE screenshots SET ocr_state='unwatched',updated_at=updated_at+1 WHERE id=?1",
            [id],
        )?;
        changed(&tx)?;
        tx.commit()?;
        Ok(())
    }

    pub fn backup(&self, destination: &Path) -> Result<()> {
        backup_database(&self.connection, destination)
    }
}

fn backup_database(connection: &Connection, destination: &Path) -> Result<()> {
    let parent = destination
        .parent()
        .context("Backup destination has no parent")?;
    let temp = tempfile::NamedTempFile::new_in(parent)?;
    connection.backup(rusqlite::MAIN_DB, temp.path(), None)?;
    temp.as_file().sync_all()?;
    temp.persist_noclobber(destination)
        .map_err(|error| error.error)
        .context("Backup destination already exists or cannot be published")?;
    #[cfg(unix)]
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}

fn normalize_source_path(path: &Path) -> std::path::PathBuf {
    fs::canonicalize(path)
        .ok()
        .or_else(|| {
            Some(
                fs::canonicalize(path.parent()?)
                    .ok()?
                    .join(path.file_name()?),
            )
        })
        .unwrap_or_else(|| path.to_owned())
}

const SHOT_SELECT: &str = "SELECT s.id,s.path,s.managed,s.captured_at,s.width,s.height,s.digest,s.available,s.ocr_state,s.ocr_error,s.updated_at,s.time_source FROM screenshots s";
fn read_shot(r: &rusqlite::Row<'_>) -> rusqlite::Result<Screenshot> {
    Ok(Screenshot {
        id: r.get(0)?,
        path: r.get(1)?,
        managed: r.get(2)?,
        captured_at: r.get(3)?,
        time_source: r.get(11)?,
        width: r.get(4)?,
        height: r.get(5)?,
        digest: r.get(6)?,
        available: r.get(7)?,
        ocr_state: r.get(8)?,
        ocr_error: r.get(9)?,
        updated_at: r.get(10)?,
    })
}
fn changed(tx: &rusqlite::Transaction<'_>) -> rusqlite::Result<usize> {
    tx.execute("UPDATE changes SET revision=revision+1 WHERE id=1", [])
}

fn search_clause(
    query: &Query,
    date: &str,
    fts: &str,
    id: &str,
    path: Option<&str>,
) -> (String, Vec<Value>, String, String) {
    let mut condition = String::new();
    let mut values = Vec::new();
    let long: Vec<_> = query
        .terms
        .iter()
        .filter(|t| t.chars().count() >= 3)
        .map(|t| format!("\"{}\"", t.replace('"', "\"\"")))
        .collect();
    if !long.is_empty() {
        condition.push_str(" AND f.text MATCH ?");
        values.push(long.join(" AND ").into());
    }
    for term in query.terms.iter().filter(|t| t.chars().count() < 3) {
        condition.push_str(&format!(
            " AND {id} IN (SELECT rowid FROM {fts} WHERE text LIKE ? ESCAPE '\\')"
        ));
        values.push(format!("%{}%", like_literal(term)).into());
    }
    if let Some(since) = query.since {
        condition.push_str(&format!(" AND {date}>=?"));
        values.push(since.into());
    }
    if let Some(until) = query.until {
        condition.push_str(&format!(" AND {date}<?"));
        values.push(until.into());
    }
    if let (Some(folder), Some(path)) = (&query.folder, path) {
        condition.push_str(&format!(
            " AND ({path} LIKE ? ESCAPE '\\' OR {path} LIKE ? ESCAPE '\\')"
        ));
        values.push(format!("{}/%", like_literal(folder.trim_end_matches(['/', '\\']))).into());
        values.push(
            format!(
                "{}\\\\%",
                like_literal(folder.trim_end_matches(['/', '\\']))
            )
            .into(),
        );
    }
    let (order, join) = if long.is_empty() {
        (String::new(), String::new())
    } else {
        ("f.rank,".into(), format!("JOIN {fts} f ON f.rowid={id}"))
    };
    (condition, values, order, join)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn migration_and_backup_preserve_canonical_data_without_overwriting() -> Result<()> {
        let root = tempfile::tempdir()?;
        let db = root.path().join("oxide.db");
        let mut store = Store::open(&db)?;
        let mut note = store.new_note()?;
        note.body = "Durable schema-one note".into();
        let note = store.save_note(&note)?;
        let image = root.path().join("shot.png");
        image::RgbImage::new(32, 32).save(&image)?;
        let shot = store.register_image(&image, true, None)?;
        store.attach(note.id, shot.id)?;
        store.connection.execute_batch(
            "ALTER TABLE screenshots DROP COLUMN retained; ALTER TABLE screenshots DROP COLUMN time_source; PRAGMA user_version=1;",
        )?;
        drop(store);
        let store = Store::open(&db)?;
        assert_eq!(store.note(note.id)?.unwrap().body, note.body);
        assert_eq!(store.attachments(note.id)?[0].id, shot.id);
        let backup = fs::read_dir(root.path())?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .find(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("oxide-before-schema-3-")
            })
            .unwrap();
        let original = Connection::open(&backup)?;
        assert_eq!(
            original.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))?,
            1
        );
        assert_eq!(
            original.query_row("SELECT body FROM notes WHERE id=?1", [note.id], |row| {
                row.get::<_, String>(0)
            })?,
            note.body
        );
        let sentinel = root.path().join("existing.db");
        fs::write(&sentinel, b"keep me")?;
        assert!(store.backup(&sentinel).is_err());
        assert_eq!(fs::read(sentinel)?, b"keep me");
        Ok(())
    }

    #[test]
    fn ocr_refresh_invalidates_same_count_caches_and_respects_removed_sources() -> Result<()> {
        let root = tempfile::tempdir()?;
        let image = root.path().join("shot.png");
        image::RgbImage::new(32, 32).save(&image)?;
        let mut store = Store::open(&root.path().join("oxide.db"))?;
        let shot = store.register_image(&image, false, None)?;
        let line = Line {
            text: "old searchable text".into(),
            x: 0.1,
            y: 0.1,
            w: 0.8,
            h: 0.1,
            confidence: None,
        };
        assert!(store.set_ocr(&shot, std::slice::from_ref(&line), "old-version")?);
        let before = store.screenshot(shot.id)?.unwrap();
        store.reindex_outdated("new-version")?;
        assert!(store.screenshot(shot.id)?.unwrap().updated_at > before.updated_at);
        assert_eq!(
            store
                .screenshots(&Query::parse("searchable")?, 10, 0)?
                .len(),
            1
        );
        let updated = Line {
            text: "replacement searchable text".into(),
            ..line
        };
        assert!(store.set_ocr(&shot, &[updated], "new-version")?);
        let before = store.screenshot(shot.id)?.unwrap();
        store.set_ocr(&shot, &[], "new-version")?;
        assert!(store.screenshot(shot.id)?.unwrap().updated_at > before.updated_at);
        store.forget_ocr(shot.id)?;
        assert!(!store.set_ocr(&shot, &[], "new-version")?);
        store.reindex(None)?;
        store.clear_derived()?;
        assert_eq!(store.screenshot(shot.id)?.unwrap().ocr_state, "unwatched");
        assert!(store.pending()?.is_none());
        let imported = store.register_import(&image)?;
        store.set_ocr(&imported, &[], "new-version")?;
        store.forget_ocr(imported.id)?;
        assert_eq!(store.screenshot(imported.id)?.unwrap().ocr_state, "no_text");
        Ok(())
    }
    #[test]
    fn authored_content_survives_restart_trash_and_cache_reset() -> Result<()> {
        let root = tempfile::tempdir()?;
        let db = root.path().join("oxide.db");
        let mut s = Store::open(&db)?;
        let mut note = s.new_note()?;
        note.title = "Meeting".into();
        note.body = "Saved decisions: 100% _ literal".into();
        let note = s.save_note(&note)?;
        s.clear_derived()?;
        s.set_note_trashed(note.id, true)?;
        drop(s);
        let mut s = Store::open(&db)?;
        assert!(s.notes(&Query::default(), false, 10, 0)?.is_empty());
        s.set_note_trashed(note.id, false)?;
        assert_eq!(s.note(note.id)?.unwrap().body, note.body);
        assert_eq!(s.notes(&Query::parse("100% _")?, false, 10, 0)?.len(), 1);
        Ok(())
    }
    #[test]
    fn future_schema_is_refused_without_reset() -> Result<()> {
        let root = tempfile::tempdir()?;
        let db = root.path().join("oxide.db");
        let mut s = Store::open(&db)?;
        let note = s.new_note()?;
        s.connection.execute_batch("PRAGMA user_version=99")?;
        drop(s);
        assert!(Store::open(&db).is_err());
        let conn = Connection::open(db)?;
        assert_eq!(
            conn.query_row("SELECT id FROM notes", [], |r| r.get::<_, i64>(0))?,
            note.id
        );
        Ok(())
    }

    #[test]
    fn invalid_image_can_be_repaired_without_changing_attachment_identity() -> Result<()> {
        let root = tempfile::tempdir()?;
        let image = root.path().join("bad.png");
        fs::write(&image, b"not an image")?;
        let mut store = Store::open(&root.path().join("oxide.db"))?;
        let shot = store.register_import(&image)?;
        assert_eq!(shot.ocr_state, "invalid");
        assert!(shot.ocr_error.is_some());
        let note = store.new_note()?;
        store.attach(note.id, shot.id)?;
        image::RgbImage::new(32, 32).save(&image)?;
        let repaired = store.register_image(&image, false, None)?;
        assert_eq!(repaired.id, shot.id);
        assert_eq!(repaired.ocr_state, "pending");
        assert_eq!(store.attachments(note.id)?[0].id, shot.id);
        Ok(())
    }

    #[test]
    fn reindex_detects_a_preserved_mtime_content_replacement() -> Result<()> {
        let root = tempfile::tempdir()?;
        let path = root.path().join("shot.png");
        image::RgbImage::from_pixel(64, 64, image::Rgb([255, 255, 255])).save(&path)?;
        let mut store = Store::open(&root.path().join("oxide.db"))?;
        let first = store.register_image(&path, false, None)?;
        let stamp = fs::metadata(&path)?.modified()?;
        image::RgbImage::from_pixel(64, 64, image::Rgb([0, 0, 0])).save(&path)?;
        fs::File::options()
            .write(true)
            .open(&path)?
            .set_times(fs::FileTimes::new().set_modified(stamp))?;
        store.reindex(Some(first.id))?;
        let updated = store.register_image(&path, false, None)?;
        assert_eq!(updated.id, first.id);
        assert_ne!(updated.digest, first.digest);
        Ok(())
    }

    #[test]
    fn imported_timestamp_can_precede_epoch_without_rounding_into_another_day() -> Result<()> {
        let root = tempfile::tempdir()?;
        let path = root.path().join("old.png");
        image::RgbImage::new(32, 32).save(&path)?;
        fs::File::options()
            .write(true)
            .open(&path)?
            .set_times(fs::FileTimes::new().set_modified(UNIX_EPOCH - Duration::from_micros(1)))?;
        let mut store = Store::open(&root.path().join("oxide.db"))?;
        let shot = store.register_import(&path)?;
        assert_eq!(shot.captured_at, -1);
        assert_eq!(shot.time_source, "modified");
        Ok(())
    }

    #[test]
    fn repeated_rename_delivery_does_not_remove_the_destination() -> Result<()> {
        let root = tempfile::tempdir()?;
        let from = root.path().join("before.png");
        let to = root.path().join("after.png");
        image::RgbImage::from_pixel(32, 32, image::Rgb([255, 255, 255])).save(&from)?;
        let mut store = Store::open(&root.path().join("oxide.db"))?;
        let shot = store.register_image(&from, false, None)?;
        let from = fs::canonicalize(from)?;
        fs::rename(&from, &to)?;
        assert!(store.rename_source(&from, &to)?);
        assert!(!store.rename_source(&from, &to)?);
        assert_eq!(store.register_image(&to, false, None)?.id, shot.id);
        Ok(())
    }

    #[test]
    fn directory_rename_preserves_linked_image_identity() -> Result<()> {
        let root = tempfile::tempdir()?;
        let from = root.path().join("original");
        fs::create_dir(&from)?;
        let image = from.join("shot.png");
        image::RgbImage::new(32, 32).save(&image)?;
        let mut store = Store::open(&root.path().join("oxide.db"))?;
        let note = store.new_note()?;
        let shot = store.register_image(&image, false, None)?;
        store.attach(note.id, shot.id)?;
        let intermediate = root.path().join("intermediate");
        let to = root.path().join("renamed");
        fs::rename(&from, &intermediate)?;
        fs::rename(&intermediate, &to)?;
        assert!(store.rename_source(&from, &intermediate)?);
        assert!(store.rename_source(&intermediate, &to)?);
        assert!(!store.rename_source(&from, &to)?);
        assert_eq!(
            store.register_image(&to.join("shot.png"), false, None)?.id,
            shot.id
        );
        assert_eq!(store.attachments(note.id)?[0].id, shot.id);
        Ok(())
    }

    #[test]
    fn literal_search_pagination_and_folder_scope_do_not_expand_input() -> Result<()> {
        let root = tempfile::tempdir()?;
        let db = root.path().join("oxide.db");
        let mut store = Store::open(&db)?;
        for index in 0..4 {
            let mut note = store.new_note()?;
            note.title = format!("literal {index}");
            note.body = "quote \" AND 100% under_score".into();
            store.save_note(&note)?;
        }
        let first = store.notes(&Query::parse("literal")?, false, 2, 0)?;
        let second = store.notes(&Query::parse("literal")?, false, 2, 2)?;
        assert_eq!(first.len() + second.len(), 4);
        assert!(!second.iter().any(|n| first.iter().any(|f| f.id == n.id)));
        assert_eq!(
            store
                .notes(&Query::parse("AND 100% under_score")?, false, 10, 0)?
                .len(),
            4
        );
        assert!(
            store
                .notes(&Query::parse("in:/images")?, false, 10, 0)?
                .is_empty()
        );
        Ok(())
    }
    #[test]
    fn attachments_and_across_line_search_survive_rename_and_reocr() -> Result<()> {
        let root = tempfile::tempdir()?;
        let image = root.path().join("shot.png");
        image::RgbImage::from_pixel(64, 64, image::Rgb([255, 255, 255])).save(&image)?;
        let mut s = Store::open(&root.path().join("oxide.db"))?;
        let note = s.new_note()?;
        let shot = s.register_image(&image, false, None)?;
        s.attach(note.id, shot.id)?;
        let line = |text: &str, y| Line {
            text: text.into(),
            x: 0.0,
            y,
            w: 0.9,
            h: 0.1,
            confidence: None,
        };
        s.set_ocr(
            &shot,
            &[line("hello 100%", 0.1), line("world _", 0.3)],
            "fixture",
        )?;
        assert_eq!(
            s.screenshots(&Query::parse("hello world")?, 10, 0)?.len(),
            1
        );
        assert_eq!(s.screenshots(&Query::parse("% _")?, 10, 0)?.len(), 1);
        assert!(s.screenshots(&Query::parse("missing")?, 10, 0)?.is_empty());
        let destination = root.path().join("renamed.png");
        fs::rename(&image, &destination)?;
        let renamed = s.register_image(&destination, false, None)?;
        assert_eq!(renamed.id, shot.id);
        s.set_ocr(&renamed, &[line("replacement", 0.1)], "fixture2")?;
        assert_eq!(s.attachments(note.id)?[0].id, shot.id);
        assert!(s.screenshots(&Query::parse("hello")?, 10, 0)?.is_empty());
        assert!(
            s.save_note(&Note {
                revision: 999,
                ..note
            })
            .is_err()
        );
        Ok(())
    }
}
