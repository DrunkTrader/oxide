use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::mpsc,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use notify::Watcher;
use oxide_core::{
    WorkerStatus,
    config::{Config, Paths, atomic_write},
    media, now,
    query::Query,
    store::Store,
};
use oxide_ocr::Reader;

fn main() {
    env_logger::init();
    if let Err(error) = run() {
        eprintln!("Oxide reader: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let mut root = None;
    let mut once = false;
    let mut ocr = None;
    let mut folder = None;
    let mut search = None;
    let mut setup = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--data-dir" => {
                root = Some(PathBuf::from(
                    args.next().context("--data-dir requires a path")?,
                ))
            }
            "--once" => once = true,
            "--ocr" => {
                ocr = Some(PathBuf::from(
                    args.next().context("--ocr requires an image")?,
                ))
            }
            "--index" => {
                folder = Some(PathBuf::from(
                    args.next().context("--index requires a folder")?,
                ));
                once = true;
            }
            "--search" => search = Some(args.next().context("--search requires a query")?),
            "--setup-models" => setup = true,
            "--help" => {
                println!(
                    "oxide-worker [--data-dir PATH] [--once | --index FOLDER | --ocr IMAGE | --search QUERY | --setup-models]"
                );
                return Ok(());
            }
            _ => bail!("Unknown reader argument: {arg}"),
        }
    }
    let paths = Paths::discover(root)?;
    let mut config = Config::load(&paths.config)?.unwrap_or_default();
    let mut store = Store::open(&paths.database())?;
    if let Some(query) = search {
        for shot in store.screenshots(&Query::parse(&query)?, 100, 0)? {
            println!("{}", shot.path);
        }
        return Ok(());
    }
    let offline = std::env::var_os("OXIDE_OFFLINE").is_some_and(|v| v == "1");
    if setup || ocr.is_some() {
        let reader = Reader::prepare(&paths.models(), config.threads, offline)?;
        if let Some(image) = ocr {
            let lines = reader.read(&media::load_image(&image)?)?;
            println!("{}", serde_json::to_string_pretty(&lines)?);
        }
        return Ok(());
    }
    background_priority();
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(paths.data.join("reader.lock"))?;
    match lock.try_lock() {
        Ok(()) => {}
        Err(std::fs::TryLockError::WouldBlock) => return Ok(()),
        Err(e) => return Err(e.into()),
    }
    store.reindex_outdated(oxide_ocr::VERSION)?;
    if let Some(folder) = folder {
        config.folders.push(fs::canonicalize(folder)?);
    }
    fs::create_dir_all(&config.capture_directory)?;
    let mut status = WorkerStatus {
        state: "Starting".into(),
        ..Default::default()
    };
    let (tx, rx) = mpsc::sync_channel(32);
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        match tx.try_send(event) {
            Ok(()) | Err(mpsc::TrySendError::Full(_)) => {}
            Err(mpsc::TrySendError::Disconnected(_)) => {}
        }
    })?;
    let mut watched = HashSet::new();
    let mut backlog = SourceScan::new(Vec::new());
    let mut source_cursor = None;
    let mut fresh = HashMap::<PathBuf, Instant>::new();
    let mut scan = true;
    let mut reader = None;
    let mut reader_threads = config.threads;
    let mut bootstrap_error = None;
    let mut heartbeat = Instant::now() - Duration::from_secs(2);
    let mut rescan = Instant::now();
    let mut backfill_at = Instant::now() - Duration::from_secs(3);
    loop {
        if !once {
            match Config::load(&paths.config) {
                Ok(Some(new)) => {
                    if config.folders != new.folders
                        || config.capture_directory != new.capture_directory
                    {
                        scan = true;
                    }
                    if new.threads != reader_threads {
                        if reader.is_some() {
                            match Reader::prepare(&paths.models(), new.threads, offline) {
                                Ok(replacement) => {
                                    reader = Some(replacement);
                                }
                                Err(error) => {
                                    status.error = Some(format!(
                                        "OCR configuration failed; previous engine retained: {error:#}"
                                    ));
                                }
                            }
                        } else {
                            bootstrap_error = None;
                        }
                        reader_threads = new.threads;
                    }
                    if config.worker_paused && !new.worker_paused {
                        bootstrap_error = None;
                    }
                    config = new;
                }
                Ok(None) => {}
                Err(e) => {
                    status.error = Some(format!("Configuration: {e}"));
                }
            }
        }
        let roots: Vec<_> = std::iter::once(config.capture_directory.clone())
            .chain(config.folders.iter().cloned())
            .map(|root| fs::canonicalize(&root).unwrap_or(root))
            .collect();
        let previous = watched
            .iter()
            .filter(|root| !roots.contains(root))
            .cloned()
            .collect::<Vec<_>>();
        for root in previous {
            if let Err(error) = watcher.unwatch(&root) {
                status.error = Some(format!("Cannot stop watching a removed source: {error}"));
            }
            watched.remove(&root);
        }
        for root in &roots {
            if root.is_dir() && !watched.contains(root) {
                match watcher.watch(root, notify::RecursiveMode::Recursive) {
                    Ok(()) => {
                        watched.insert(root.clone());
                    }
                    Err(e) => status.error = Some(format!("Cannot watch folder: {e}")),
                }
            }
        }
        if scan
            || (rescan.elapsed() >= Duration::from_secs(30)
                && backlog.finished()
                && source_cursor.is_none())
        {
            backlog = SourceScan::new(roots.clone());
            source_cursor = Some(0);
            scan = false;
            rescan = Instant::now();
        }
        for event in rx.try_iter() {
            match event {
                Ok(event) => {
                    if matches!(
                        event.kind,
                        notify::EventKind::Modify(notify::event::ModifyKind::Name(
                            notify::event::RenameMode::Both
                        ))
                    ) && event.paths.len() == 2
                        && let Err(error) = store.rename_source(&event.paths[0], &event.paths[1])
                    {
                        status.error = Some(format!("Source rename: {error:#}"));
                    }
                    for path in event.paths {
                        if media::supported(&path) && path.is_file() {
                            if fresh.len() < 1024 {
                                fresh.insert(path, Instant::now());
                            } else {
                                scan = true;
                            }
                        } else {
                            scan = true;
                        }
                    }
                }
                Err(e) => {
                    status.error = Some(format!("File events: {e}"));
                    scan = true;
                }
            }
        }
        if !config.worker_paused {
            let settled = fresh
                .iter()
                .filter(|(_, time)| time.elapsed() >= Duration::from_millis(400))
                .map(|(path, _)| path.clone())
                .collect::<Vec<_>>();
            for path in settled {
                fresh.remove(&path);
                match import_image(&mut store, &paths, &path, path.starts_with(&roots[0])) {
                    Ok(()) => {}
                    Err(error) => {
                        let recent = fs::metadata(&path)
                            .and_then(|m| m.modified())
                            .ok()
                            .is_some_and(|stamp| {
                                stamp
                                    .elapsed()
                                    .is_ok_and(|age| age < Duration::from_secs(10))
                            });
                        if recent {
                            fresh.insert(path, Instant::now());
                        } else {
                            status.error = Some(format!("Image import failed: {error}"));
                        }
                    }
                }
            }
        }
        if !config.worker_paused
            && let Some(cursor) = source_cursor
        {
            let rows = store.source_rows(cursor)?;
            source_cursor = rows.last().map(|row| row.0);
            for (id, path, managed, retained) in rows {
                let path = Path::new(&path);
                if !managed && !retained && !roots.iter().any(|root| path.starts_with(root)) {
                    store.forget_ocr(id)?;
                } else if retained && path.is_file() {
                    if let Err(error) = import_image(&mut store, &paths, path, managed) {
                        status.error = Some(format!("Manual import reconciliation: {error:#}"));
                    }
                } else if (retained && path.parent().is_some_and(Path::is_dir))
                    || roots
                        .iter()
                        .any(|root| root.is_dir() && path.starts_with(root))
                {
                    match path.try_exists() {
                        Ok(false) => store.set_available(id, false)?,
                        Err(error) => {
                            status.error =
                                Some(format!("Source availability could not be checked: {error}"))
                        }
                        _ => {}
                    }
                }
            }
        }
        if config.worker_paused {
            status.state = "Paused".into();
        } else if let Some(pending) = store.pending()? {
            let shot = match store.register_image(Path::new(&pending.path), pending.managed, None) {
                Ok(shot) if shot.ocr_state != "invalid" => Some(shot),
                Ok(shot) => {
                    status.error = shot.ocr_error;
                    None
                }
                Err(error) => {
                    store.ocr_failed(pending.id, &format!("{error:#}"))?;
                    None
                }
            };
            if shot.is_some() && reader.is_none() && bootstrap_error.is_none() {
                status.state = "Preparing local OCR models".into();
                write_status(&paths, &mut status)?;
                match Reader::prepare(&paths.models(), config.threads, offline) {
                    Ok(engine) => {
                        reader = Some(engine);
                    }
                    Err(e) => {
                        bootstrap_error = Some(format!("{e:#}"));
                    }
                }
            }
            if let (Some(reader), Some(shot)) = (&reader, &shot) {
                status.state = "Recognizing screenshot".into();
                write_status(&paths, &mut status)?;
                match media::load_image(Path::new(&shot.path)).and_then(|image| {
                    media::thumbnail(&paths, shot.id, &image, Path::new(&shot.path))?;
                    let lines = reader.read(&image)?;
                    store.set_ocr(shot, &lines, oxide_ocr::VERSION)?;
                    Ok(())
                }) {
                    Ok(()) => {
                        status.indexed += 1;
                    }
                    Err(e) => {
                        store.ocr_failed(shot.id, &format!("{e:#}"))?;
                        status.error = Some(format!("Screenshot OCR failed: {e}"));
                    }
                }
            } else if shot.is_some() {
                status.state = "OCR unavailable".into();
                status.error = bootstrap_error.clone();
            }
        }
        if !config.worker_paused
            && (!on_battery() || backfill_at.elapsed() >= Duration::from_secs(3))
            && let Some(item) = backlog.next()
        {
            backfill_at = Instant::now();
            match item.and_then(|path| {
                import_image(&mut store, &paths, &path, path.starts_with(&roots[0]))
            }) {
                Ok(()) => {}
                Err(e) => {
                    status.error = Some(format!("Image import failed: {e}"));
                }
            }
        }
        status.backlog = store.pending_count()?;
        if backlog.finished()
            && source_cursor.is_none()
            && store.pending()?.is_none()
            && !config.worker_paused
        {
            status.state = "Up to date".into();
        }
        if heartbeat.elapsed() >= Duration::from_secs(1) {
            write_status(&paths, &mut status)?;
            heartbeat = Instant::now();
        }
        if once && backlog.finished() && source_cursor.is_none() {
            if let Some(error) = bootstrap_error {
                write_status(&paths, &mut status)?;
                bail!("{error}");
            }
            if config.worker_paused || store.pending()?.is_none() {
                write_status(&paths, &mut status)?;
                return Ok(());
            }
        }
        if config.worker_paused
            || (backlog.finished() && source_cursor.is_none() && store.pending()?.is_none())
            || bootstrap_error.is_some()
            || (!backlog.finished() && on_battery())
        {
            std::thread::sleep(Duration::from_millis(300));
        }
    }
}

fn background_priority() {
    #[cfg(target_os = "linux")]
    {
        // SAFETY: process 0 is this reader; lowering its scheduling priority
        // requires no external memory and does not affect the UI process.
        if unsafe { libc::setpriority(libc::PRIO_PROCESS, 0, 10) } != 0 {
            log::debug!("Reader priority: {}", std::io::Error::last_os_error());
        }
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Threading::{
            BELOW_NORMAL_PRIORITY_CLASS, GetCurrentProcess, SetPriorityClass,
        };
        // SAFETY: the pseudo handle identifies this live reader process.
        if unsafe { SetPriorityClass(GetCurrentProcess(), BELOW_NORMAL_PRIORITY_CLASS) } == 0 {
            log::debug!("Reader priority: {}", std::io::Error::last_os_error());
        }
    }
}

fn on_battery() -> bool {
    #[cfg(target_os = "linux")]
    {
        let Ok(entries) = fs::read_dir("/sys/class/power_supply") else {
            return false;
        };
        for entry in entries.flatten() {
            if fs::read_to_string(entry.path().join("type"))
                .is_ok_and(|text| text.trim() == "Battery")
                && fs::read_to_string(entry.path().join("status"))
                    .is_ok_and(|text| text.trim() == "Discharging")
            {
                return true;
            }
        }
        false
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
        let mut status = SYSTEM_POWER_STATUS::default();
        // SAFETY: status is an initialized, writable structure with the API's layout.
        unsafe { GetSystemPowerStatus(&mut status) != 0 && status.ACLineStatus == 0 }
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        false
    }
}

fn import_image(store: &mut Store, paths: &Paths, path: &Path, managed: bool) -> Result<()> {
    paths.ensure_original_location(path)?;
    let shot = store.register_image(path, managed, None)?;
    if shot.ocr_state == "invalid" {
        bail!(
            "Invalid image {}: {}",
            path.display(),
            shot.ocr_error.as_deref().unwrap_or("Cannot decode image")
        );
    }
    if !paths.thumbnail(shot.id).exists() || shot.ocr_state == "pending" {
        media::thumbnail(paths, shot.id, &media::load_image(path)?, path)?;
    }
    Ok(())
}

fn write_status(paths: &Paths, status: &mut WorkerStatus) -> Result<()> {
    status.updated_at = now();
    atomic_write(
        &paths.data.join("reader.json"),
        &serde_json::to_vec(status)?,
    )
}

/// Streaming backfill: at most 128 open directory iterators, not a library-sized file queue.
struct SourceScan {
    roots: std::vec::IntoIter<PathBuf>,
    stack: Vec<fs::ReadDir>,
}

impl SourceScan {
    fn new(roots: Vec<PathBuf>) -> Self {
        // ponytail: O(n²) configured-root deduplication; use a sorted prefix pass if root lists become large.
        let unique = roots
            .iter()
            .enumerate()
            .filter(|(index, root)| {
                !roots.iter().enumerate().any(|(other, parent)| {
                    other != *index
                        && root.starts_with(parent)
                        && (root != &parent || other < *index)
                })
            })
            .map(|(_, root)| root.clone())
            .collect::<Vec<_>>();
        Self {
            roots: unique.into_iter(),
            stack: Vec::new(),
        }
    }
    fn finished(&self) -> bool {
        self.roots.len() == 0 && self.stack.is_empty()
    }
}

impl Iterator for SourceScan {
    type Item = Result<PathBuf>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let Some(entries) = self.stack.last_mut() else {
                let root = self.roots.next()?;
                match fs::read_dir(&root) {
                    Ok(entries) => self.stack.push(entries),
                    Err(error) => {
                        return Some(
                            Err(error).with_context(|| format!("Cannot scan {}", root.display())),
                        );
                    }
                }
                continue;
            };
            let entry = match entries.next() {
                Some(Ok(entry)) => entry,
                Some(Err(error)) => return Some(Err(error.into())),
                None => {
                    self.stack.pop();
                    continue;
                }
            };
            let kind = match entry.file_type() {
                Ok(kind) => kind,
                Err(error) => return Some(Err(error.into())),
            };
            let path = entry.path();
            if kind.is_dir() && !entry.file_name().to_string_lossy().starts_with('.') {
                if self.stack.len() >= 128 {
                    return Some(Err(anyhow::anyhow!(
                        "Directory nesting exceeds the 128-level scan limit: {}",
                        path.display()
                    )));
                }
                match fs::read_dir(&path) {
                    Ok(entries) => self.stack.push(entries),
                    Err(error) => {
                        return Some(
                            Err(error).with_context(|| format!("Cannot scan {}", path.display())),
                        );
                    }
                }
            } else if kind.is_file() && media::supported(&path) {
                return Some(Ok(path));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overlapping_roots_stream_once_and_do_not_follow_directory_links() -> Result<()> {
        let root = tempfile::tempdir()?;
        let folder = root.path().join("nested");
        fs::create_dir(&folder)?;
        fs::write(root.path().join("one.png"), b"fixture")?;
        fs::write(folder.join("two.jpg"), b"fixture")?;
        #[cfg(unix)]
        std::os::unix::fs::symlink(root.path(), folder.join("cycle"))?;
        let paths = SourceScan::new(vec![root.path().to_owned(), folder, root.path().to_owned()])
            .collect::<Result<Vec<_>>>()?;
        assert_eq!(paths.len(), 2);
        Ok(())
    }
}
