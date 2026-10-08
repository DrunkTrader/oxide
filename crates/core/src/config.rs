use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use directories::{ProjectDirs, UserDirs};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Paths {
    pub data: PathBuf,
    pub cache: PathBuf,
    pub config: PathBuf,
}

impl Paths {
    pub fn discover(override_root: Option<PathBuf>) -> Result<Self> {
        let mut paths = if let Some(root) = override_root {
            fs::create_dir_all(&root)?;
            let root = fs::canonicalize(root)?;
            let descriptor = root.join("paths.toml");
            if descriptor.exists() {
                let paths: Self = toml::from_str(&fs::read_to_string(descriptor)?)
                    .context("Invalid Oxide directory descriptor")?;
                if fs::canonicalize(&paths.data)? != root
                    || !paths.config.is_absolute()
                    || !paths.cache.is_absolute()
                {
                    bail!("Oxide directory descriptor does not belong to this workspace");
                }
                paths
            } else {
                Self {
                    data: root.clone(),
                    cache: root.join("cache"),
                    config: root.join("config.toml"),
                }
            }
        } else {
            let dirs = ProjectDirs::from("app", "Oxide", "Oxide")
                .context("Cannot locate user application directories")?;
            Self {
                data: dirs.data_local_dir().into(),
                cache: dirs.cache_dir().into(),
                config: dirs.config_dir().join("config.toml"),
            }
        };
        fs::create_dir_all(&paths.data).context("Cannot create Oxide data directory")?;
        fs::create_dir_all(&paths.cache).context("Cannot create Oxide cache directory")?;
        if let Some(parent) = paths.config.parent() {
            fs::create_dir_all(parent)?;
        }
        paths.data = fs::canonicalize(&paths.data)?;
        paths.cache = fs::canonicalize(&paths.cache)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&paths.data, fs::Permissions::from_mode(0o700))?;
        }
        atomic_write(
            &paths.data.join("paths.toml"),
            toml::to_string(&paths)?.as_bytes(),
        )?;
        Ok(paths)
    }

    pub fn database(&self) -> PathBuf {
        self.data.join("oxide.db")
    }
    pub fn thumbnail(&self, id: i64) -> PathBuf {
        self.cache.join("thumbs").join(format!("{id}.jpg"))
    }
    pub fn models(&self) -> PathBuf {
        self.data.join("models")
    }
    pub fn ensure_original_location(&self, path: &Path) -> Result<()> {
        let resolved = fs::canonicalize(path).unwrap_or_else(|_| path.to_owned());
        if resolved.starts_with(&self.cache) {
            bail!(
                "Screenshot originals and watched folders must be outside Oxide's derived cache directory"
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub recording_exclusion_enabled: bool,
    pub capture_directory: PathBuf,
    pub folders: Vec<PathBuf>,
    pub theme: String,
    pub reduce_motion: bool,
    pub always_on_top: bool,
    pub start_at_login: bool,
    pub last_note: Option<i64>,
    pub threads: usize,
    pub worker_paused: bool,
    pub summon_shortcut: String,
    pub hide_shortcut: String,
    pub capture_shortcut: String,
    pub display_shortcut: String,
}

impl Default for Config {
    fn default() -> Self {
        let base = UserDirs::new()
            .and_then(|d| d.picture_dir().map(Path::to_path_buf))
            .or_else(|| UserDirs::new().map(|d| d.home_dir().join("Pictures")))
            .unwrap_or_else(std::env::temp_dir);
        Self {
            recording_exclusion_enabled: cfg!(windows),
            capture_directory: base.join("Oxide"),
            folders: Vec::new(),
            theme: "system".into(),
            reduce_motion: false,
            always_on_top: false,
            start_at_login: false,
            last_note: None,
            threads: std::thread::available_parallelism().map_or(2, |n| n.get().min(4)),
            worker_paused: false,
            summon_shortcut: "Alt+Shift+O".into(),
            hide_shortcut: "Alt+Shift+H".into(),
            capture_shortcut: "Alt+Shift+R".into(),
            display_shortcut: "Alt+Shift+D".into(),
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Option<Self>> {
        match fs::read_to_string(path) {
            Ok(text) => {
                let config: Self = toml::from_str(&text)
                    .context("Invalid Oxide configuration; the existing file was preserved")?;
                config.validate()?;
                Ok(Some(config))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e).context("Cannot read Oxide configuration"),
        }
    }

    pub fn validate(&self) -> Result<()> {
        if !(1..=16).contains(&self.threads) {
            bail!("OCR threads must be between 1 and 16");
        }
        if !["system", "light", "dark"].contains(&self.theme.as_str()) {
            bail!("Theme must be system, light, or dark");
        }
        if !self.capture_directory.is_absolute() {
            bail!("Capture directory must be an absolute path");
        }
        for root in &self.folders {
            if !root.is_absolute()
                || root.parent().is_none()
                || UserDirs::new().is_some_and(|d| root == d.home_dir())
            {
                bail!(
                    "Choose a specific screenshot folder rather than a filesystem root or home directory"
                );
            }
        }
        Ok(())
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        atomic_write(path, toml::to_string_pretty(self)?.as_bytes())
    }
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .context("Destination has no parent directory")?;
    fs::create_dir_all(parent)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    temp.persist(path)
        .map_err(|e| e.error)
        .context("Cannot publish file atomically")?;
    #[cfg(unix)]
    {
        fs::File::open(parent)?.sync_all()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exclusion_preference_survives_restart() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("config.toml");
        let mut c = Config {
            recording_exclusion_enabled: true,
            reduce_motion: true,
            ..Default::default()
        };
        c.save(&path)?;
        let loaded = Config::load(&path)?.unwrap();
        assert!(loaded.recording_exclusion_enabled);
        assert!(loaded.reduce_motion);
        c.recording_exclusion_enabled = false;
        c.save(&path)?;
        assert!(!Config::load(&path)?.unwrap().recording_exclusion_enabled);
        Ok(())
    }
    #[test]
    fn malformed_configuration_is_not_unconfigured() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("config.toml");
        fs::write(&path, "not = [valid")?;
        assert!(Config::load(&path).is_err());
        assert_eq!(fs::read_to_string(path)?, "not = [valid");
        Ok(())
    }

    #[test]
    fn subprocess_uses_the_same_configuration_and_cache_paths() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let paths = Paths::discover(Some(temp.path().join("data")))?;
        let separate = Paths {
            config: temp.path().join("separate-config.toml"),
            cache: temp.path().join("separate-cache"),
            ..paths.clone()
        };
        fs::create_dir_all(&separate.cache)?;
        atomic_write(
            &paths.data.join("paths.toml"),
            toml::to_string(&separate)?.as_bytes(),
        )?;
        let child = Paths::discover(Some(paths.data))?;
        assert_eq!(child.config, separate.config);
        assert_eq!(child.cache, separate.cache);
        Ok(())
    }
}
