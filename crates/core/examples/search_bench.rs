//! Synthetic search latency check; does not benchmark OCR or claim GUI latency.
use anyhow::{Result, ensure};
use oxide_core::{query::Query, store::Store};
use rusqlite::params;
use std::time::Instant;

fn main() -> Result<()> {
    let root = tempfile::tempdir()?;
    let db = root.path().join("bench.db");
    let store = Store::open(&db)?;
    let mut connection = rusqlite::Connection::open(&db)?;
    let tx = connection.transaction()?;
    for id in 1..=10000 {
        tx.execute("INSERT INTO screenshots(id,path,managed,captured_at,time_source,mtime_ns,file_size,width,height,digest,ocr_state,updated_at) VALUES(?1,?2,0,?3,'modified','1',1000,1080,640,'synthetic','ready',?3)",params![id,format!("/bench/screenshots/{id}.png"),oxide_core::now()])?;
        tx.execute(
            "INSERT INTO screenshots_fts(rowid,text) VALUES(?1,?2)",
            params![
                id,
                format!(
                    "Oxide synthetic screenshot {id}\nDatabase error {}\nHTTP response 100%",
                    id % 97
                )
            ],
        )?;
    }
    for id in 1..=1000 {
        tx.execute(
            "INSERT INTO notes(id,title,body,created_at,updated_at) VALUES(?1,?2,?3,?4,?4)",
            params![
                id,
                format!("Note {id}"),
                "Database error notes with decisions",
                oxide_core::now()
            ],
        )?;
        tx.execute(
            "INSERT INTO notes_fts(rowid,text) VALUES(?1,?2)",
            params![
                id,
                format!("Note {id}\nDatabase error notes with decisions")
            ],
        )?;
    }
    tx.commit()?;
    for text in [
        "database error",
        "100%",
        "o",
        "in:/bench/screenshots database",
        "in:/bench/screenshots",
        "after:1970-01-01 before:2100-01-01",
        "",
    ] {
        let query = Query::parse(text)?;
        let mut times = Vec::new();
        for _ in 0..100 {
            let start = Instant::now();
            let shots = store.screenshots(&query, 50, 0)?;
            let _notes = store.notes(&query, false, 50, 0)?;
            ensure!(
                !shots.is_empty(),
                "Benchmark query unexpectedly returned no screenshots"
            );
            times.push(start.elapsed());
        }
        times.sort();
        println!(
            "query={text:?} p50={:?} p95={:?} corpus=10000 screenshots + 1000 notes",
            times[50], times[95]
        );
    }
    Ok(())
}
