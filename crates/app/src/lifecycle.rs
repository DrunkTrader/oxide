use std::{
    fs::{self, File},
    io::{BufRead, BufReader, Read, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        Arc, OnceLock,
        mpsc::{self, Receiver, SyncSender},
    },
    time::Duration,
};

use anyhow::{Context, Result, bail};
use eframe::egui;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState, hotkey::HotKey};
use oxide_core::config::{Config, Paths};

#[derive(Clone, Copy, Debug)]
pub enum Action {
    Show,
    Toggle,
    Hide,
    Region,
    Display,
    Quit,
    Normal,
}

pub struct Resident {
    _lock: File,
    pub receiver: Receiver<Action>,
    pub sender: SyncSender<Action>,
    context: Arc<OnceLock<egui::Context>>,
    endpoint: PathBuf,
}

impl Resident {
    pub fn claim(paths: &Paths, normal: bool) -> Result<Option<Self>> {
        let lock = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(paths.data.join("resident.lock"))?;
        match lock.try_lock() {
            Ok(()) => {}
            Err(std::fs::TryLockError::WouldBlock) => {
                let mut last = None;
                for _ in 0..40 {
                    match wake(paths, normal) {
                        Ok(()) => return Ok(None),
                        Err(e) => last = Some(e),
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                return Err(last.unwrap_or_else(|| anyhow::anyhow!("Resident window is not ready")));
            }
            Err(e) => return Err(e.into()),
        }
        let (sender, receiver) = mpsc::sync_channel(32);
        let context = Arc::new(OnceLock::<egui::Context>::new());
        let thread_sender = sender.clone();
        let thread_context = context.clone();
        #[cfg(unix)]
        let endpoint = {
            use std::os::unix::fs::PermissionsExt;
            use std::os::unix::net::UnixListener;
            let endpoint = paths.data.join("resident.sock");
            if endpoint.exists() {
                fs::remove_file(&endpoint)?;
            }
            let listener = UnixListener::bind(&endpoint)?;
            fs::set_permissions(&endpoint, fs::Permissions::from_mode(0o600))?;
            std::thread::Builder::new()
                .name("oxide-resident".into())
                .spawn(move || {
                    for incoming in listener.incoming() {
                        match incoming {
                            Ok(mut stream) => {
                                if let Err(e) = stream
                                    .set_read_timeout(Some(Duration::from_secs(1)))
                                    .and_then(|_| {
                                        stream.set_write_timeout(Some(Duration::from_secs(1)))
                                    })
                                {
                                    log::warn!("Resident connection deadline: {e}");
                                    continue;
                                }
                                match read_message(&mut stream) {
                                    Ok(message) if message == "show" || message == "normal" => {
                                        if thread_sender
                                            .try_send(if message == "normal" {
                                                Action::Normal
                                            } else {
                                                Action::Show
                                            })
                                            .is_ok()
                                        {
                                            if let Some(ctx) = thread_context.get() {
                                                ctx.request_repaint();
                                            }
                                            if let Err(e) = stream.write_all(b"ok\n") {
                                                log::debug!("Wake acknowledgement: {e}");
                                            }
                                        }
                                    }
                                    Ok(_) => {}
                                    Err(e) => log::debug!("Rejected wake message: {e}"),
                                }
                            }
                            Err(e) => {
                                log::warn!("Resident listener: {e}");
                                break;
                            }
                        }
                    }
                })?;
            endpoint
        };
        #[cfg(windows)]
        let endpoint = {
            use std::net::TcpListener;
            let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
            let mut random = [0_u8; 32];
            getrandom::fill(&mut random)?;
            let token = random
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>();
            let endpoint = paths.data.join("resident.json");
            oxide_core::config::atomic_write(
                &endpoint,
                &serde_json::to_vec(
                    &serde_json::json!({"port":listener.local_addr()?.port(),"token":token}),
                )?,
            )?;
            std::thread::Builder::new()
                .name("oxide-resident".into())
                .spawn(move || {
                    for incoming in listener.incoming() {
                        let Ok(mut stream) = incoming else {
                            break;
                        };
                        if let Err(e) = stream
                            .set_read_timeout(Some(Duration::from_secs(1)))
                            .and_then(|_| stream.set_write_timeout(Some(Duration::from_secs(1))))
                        {
                            log::warn!("Resident connection deadline: {e}");
                            continue;
                        }
                        match read_message(&mut stream) {
                            Ok(message)
                                if message == format!("{token} show")
                                    || message == format!("{token} normal") =>
                            {
                                if thread_sender
                                    .try_send(if message.ends_with(" normal") {
                                        Action::Normal
                                    } else {
                                        Action::Show
                                    })
                                    .is_ok()
                                {
                                    if let Some(ctx) = thread_context.get() {
                                        ctx.request_repaint();
                                    }
                                    if let Err(e) = stream.write_all(b"ok\n") {
                                        log::debug!("Wake acknowledgement: {e}");
                                    }
                                }
                            }
                            Ok(_) => {}
                            Err(e) => log::debug!("Rejected wake message: {e}"),
                        }
                    }
                })?;
            endpoint
        };
        Ok(Some(Self {
            _lock: lock,
            receiver,
            sender,
            context,
            endpoint,
        }))
    }
    pub fn attach_context(&self, context: egui::Context) {
        if self.context.set(context).is_err() {
            log::debug!("Resident context already attached");
        }
    }
    pub fn context(&self) -> Option<egui::Context> {
        self.context.get().cloned()
    }
}

impl Drop for Resident {
    fn drop(&mut self) {
        if let Err(e) = fs::remove_file(&self.endpoint)
            && e.kind() != std::io::ErrorKind::NotFound
        {
            log::debug!("Resident endpoint cleanup: {e}");
        }
    }
}

fn read_message(stream: &mut impl Read) -> Result<String> {
    let mut message = String::new();
    BufReader::new(stream.take(129)).read_line(&mut message)?;
    if message.len() > 128 || !message.ends_with('\n') {
        bail!("Invalid or oversized wake message");
    }
    Ok(message.trim_end().to_owned())
}

fn wake(paths: &Paths, normal: bool) -> Result<()> {
    #[cfg(unix)]
    let mut stream = std::os::unix::net::UnixStream::connect(paths.data.join("resident.sock"))?;
    #[cfg(windows)]
    let (mut stream, token) = {
        let data: serde_json::Value =
            serde_json::from_slice(&fs::read(paths.data.join("resident.json"))?)?;
        let port = data["port"]
            .as_u64()
            .context("Invalid resident port")?
            .try_into()?;
        let token = data["token"]
            .as_str()
            .context("Invalid resident token")?
            .to_owned();
        (
            std::net::TcpStream::connect_timeout(
                &std::net::SocketAddr::from((std::net::Ipv4Addr::LOCALHOST, port)),
                Duration::from_secs(1),
            )?,
            token,
        )
    };
    stream.set_read_timeout(Some(Duration::from_secs(1)))?;
    stream.set_write_timeout(Some(Duration::from_secs(1)))?;
    let command = if normal { "normal" } else { "show" };
    #[cfg(unix)]
    stream.write_all(format!("{command}\n").as_bytes())?;
    #[cfg(windows)]
    stream.write_all(format!("{token} {command}\n").as_bytes())?;
    if read_message(&mut stream)? != "ok" {
        bail!("Resident did not acknowledge window wake");
    }
    Ok(())
}

pub struct Hotkeys {
    manager: GlobalHotKeyManager,
    keys: Vec<HotKey>,
}

impl Hotkeys {
    pub fn new(
        config: &Config,
        sender: SyncSender<Action>,
        context: egui::Context,
    ) -> Result<Self> {
        let mut hotkeys = Self {
            manager: GlobalHotKeyManager::new()?,
            keys: Vec::new(),
        };
        hotkeys.update(config, sender, context)?;
        Ok(hotkeys)
    }
    pub fn validate(config: &Config) -> Result<Vec<(HotKey, Action)>> {
        let mut bindings = Vec::new();
        for (text, action) in [
            (&config.summon_shortcut, Action::Toggle),
            (&config.hide_shortcut, Action::Hide),
            (&config.capture_shortcut, Action::Region),
            (&config.display_shortcut, Action::Display),
        ] {
            let key: HotKey = text
                .parse()
                .with_context(|| format!("Invalid shortcut: {text}"))?;
            if bindings
                .iter()
                .any(|(existing, _): &(HotKey, Action)| existing.id() == key.id())
            {
                bail!("Global shortcut assignments must be distinct");
            }
            bindings.push((key, action));
        }
        Ok(bindings)
    }
    pub fn update(
        &mut self,
        config: &Config,
        sender: SyncSender<Action>,
        context: egui::Context,
    ) -> Result<()> {
        let bindings = Self::validate(config)?;
        let old = self.keys.clone();
        self.manager.unregister_all(&old)?;
        let mut registered = Vec::new();
        for (key, _) in &bindings {
            if let Err(error) = self.manager.register(*key) {
                for registered_key in &registered {
                    if let Err(e) = self.manager.unregister(*registered_key) {
                        log::warn!("Shortcut rollback: {e}");
                    }
                }
                for old_key in &old {
                    if let Err(e) = self.manager.register(*old_key) {
                        log::warn!("Shortcut restoration: {e}");
                    }
                }
                return Err(error).context("Global shortcut conflicts with another application; previous bindings were retained where possible");
            }
            registered.push(*key);
        }
        self.keys = registered;
        GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
            if event.state == HotKeyState::Pressed
                && let Some((_, action)) = bindings.iter().find(|(key, _)| key.id() == event.id)
                && sender.try_send(*action).is_ok()
            {
                context.request_repaint();
            }
        }));
        Ok(())
    }
}

pub struct Worker {
    child: Option<Child>,
    paths: Paths,
}

impl Drop for Hotkeys {
    fn drop(&mut self) {
        if let Err(error) = self.manager.unregister_all(&self.keys) {
            log::debug!("Shortcut shutdown: {error}");
        }
    }
}
impl Worker {
    pub fn new(paths: Paths) -> Self {
        Self { child: None, paths }
    }
    pub fn start(&mut self) -> Result<()> {
        if let Some(child) = &mut self.child
            && child.try_wait()?.is_none()
        {
            return Ok(());
        }
        let binary = std::env::current_exe()?.with_file_name(if cfg!(windows) {
            "oxide-worker.exe"
        } else {
            "oxide-worker"
        });
        if !binary.is_file() {
            bail!(
                "Reader executable is missing. Build/install the complete Cargo workspace, then choose Restart reader"
            );
        }
        let log = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.paths.data.join("reader.log"))?;
        self.child = Some(
            Command::new(binary)
                .arg("--data-dir")
                .arg(&self.paths.data)
                .stdout(Stdio::null())
                .stderr(log)
                .spawn()
                .context("Cannot start local OCR reader")?,
        );
        Ok(())
    }

    pub fn restart(&mut self) -> Result<()> {
        if let Some(mut child) = self.child.take() {
            if child.try_wait()?.is_none() {
                child.kill()?;
            }
            child.wait()?;
        }
        self.start()
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child {
            if let Err(e) = child.kill()
                && e.kind() != std::io::ErrorKind::InvalidInput
            {
                log::debug!("Reader shutdown: {e}");
            }
            if let Err(e) = child.wait() {
                log::debug!("Reader wait: {e}");
            }
        }
    }
}

#[cfg(windows)]
pub struct Tray {
    _icon: tray_icon::TrayIcon,
}

#[cfg(windows)]
impl Tray {
    pub fn new(sender: SyncSender<Action>, context: egui::Context) -> Result<Self> {
        use tray_icon::{
            TrayIconBuilder, TrayIconEvent,
            menu::{Menu, MenuEvent, MenuItem},
        };
        let menu = Menu::new();
        let show = MenuItem::new("Show Oxide", true, None);
        let hide = MenuItem::new("Hide Oxide", true, None);
        let normal = MenuItem::new("Recording exclusion Off", true, None);
        let quit = MenuItem::new("Quit", true, None);
        menu.append_items(&[&show, &hide, &normal, &quit])?;
        let ids = [
            (show.id().clone(), Action::Show),
            (hide.id().clone(), Action::Hide),
            (normal.id().clone(), Action::Normal),
            (quit.id().clone(), Action::Quit),
        ];
        let icon = crate::icon();
        let icon = tray_icon::Icon::from_rgba(icon.rgba, icon.width, icon.height)?;
        let icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_icon(icon)
            .with_tooltip("Oxide")
            .build()?;
        let menu_sender = sender.clone();
        let menu_context = context.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            if let Some((_, action)) = ids.iter().find(|(id, _)| *id == event.id)
                && menu_sender.try_send(*action).is_ok()
            {
                menu_context.request_repaint();
            }
        }));
        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
            if matches!(event, TrayIconEvent::DoubleClick { .. })
                && sender.try_send(Action::Show).is_ok()
            {
                context.request_repaint();
            }
        }));
        Ok(Self { _icon: icon })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resident_messages_are_bounded_and_explicit() {
        assert_eq!(read_message(&mut &b"show\n"[..]).unwrap(), "show");
        assert!(read_message(&mut &b"show"[..]).is_err());
        assert!(read_message(&mut "x".repeat(200).as_bytes()).is_err());
    }
    #[test]
    fn duplicate_shortcut_bindings_are_rejected() {
        let config = Config {
            hide_shortcut: "Alt+Shift+O".into(),
            ..Default::default()
        };
        assert!(Hotkeys::validate(&config).is_err());
    }
}
