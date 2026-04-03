use std::env;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PilcrowConfig {
    #[serde(default)]
    pub web: WebConfig,
    #[serde(default)]
    pub backend: BackendConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WebConfig {
    #[serde(default = "default_web_host")]
    pub host: String,
    #[serde(default = "default_web_port")]
    pub port: u16,
    #[serde(default = "default_backend_url")]
    pub backend_url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackendConfig {
    #[serde(default = "default_backend_host")]
    pub host: String,
    #[serde(default = "default_backend_port")]
    pub port: u16,
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            host: default_web_host(),
            port: default_web_port(),
            backend_url: default_backend_url(),
        }
    }
}

impl Default for BackendConfig {
    fn default() -> Self {
        Self {
            host: default_backend_host(),
            port: default_backend_port(),
        }
    }
}

impl PilcrowConfig {
    pub fn load_from(start_dir: impl AsRef<Path>) -> io::Result<Self> {
        let start_dir = start_dir.as_ref();
        let mut config = match find_config_path(start_dir) {
            Some(path) => {
                let raw = std::fs::read_to_string(&path)?;
                toml::from_str::<Self>(&raw).map_err(|err| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("failed to parse {}: {err}", path.display()),
                    )
                })?
            }
            None => Self::default(),
        };
        config.apply_env_overrides()?;
        Ok(config)
    }

    pub fn load_from_current_dir() -> io::Result<Self> {
        let cwd = env::current_dir()?;
        Self::load_from(cwd)
    }

    pub fn web_bind_addr(&self) -> String {
        format!("{}:{}", self.web.host, self.web.port)
    }

    pub fn backend_bind_addr(&self) -> String {
        format!("{}:{}", self.backend.host, self.backend.port)
    }

    fn apply_env_overrides(&mut self) -> io::Result<()> {
        if let Some(host) = get_env("PILCROW_WEB_HOST")? {
            self.web.host = host;
        }
        if let Some(port) = get_env_u16("PILCROW_WEB_PORT")? {
            self.web.port = port;
        }
        if let Some(url) = get_env("PILCROW_BACKEND_URL")? {
            self.web.backend_url = url;
        }
        if let Some(host) = get_env("PILCROW_BACKEND_HOST")? {
            self.backend.host = host;
        }
        if let Some(port) = get_env_u16("PILCROW_BACKEND_PORT")? {
            self.backend.port = port;
        }
        Ok(())
    }
}

fn find_config_path(start_dir: &Path) -> Option<PathBuf> {
    let mut current = Some(start_dir);
    while let Some(dir) = current {
        let candidate = dir.join("Pilcrow.toml");
        if candidate.exists() {
            return Some(candidate);
        }
        current = dir.parent();
    }
    None
}

fn get_env(key: &str) -> io::Result<Option<String>> {
    match env::var(key) {
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("environment variable {key} is not valid unicode"),
        )),
    }
}

fn get_env_u16(key: &str) -> io::Result<Option<u16>> {
    match get_env(key)? {
        Some(raw) => raw.parse::<u16>().map(Some).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("environment variable {key} must be a valid port: {err}"),
            )
        }),
        None => Ok(None),
    }
}

fn default_web_host() -> String {
    "127.0.0.1".to_string()
}

fn default_web_port() -> u16 {
    3000
}

fn default_backend_url() -> String {
    "http://127.0.0.1:4000".to_string()
}

fn default_backend_host() -> String {
    "127.0.0.1".to_string()
}

fn default_backend_port() -> u16 {
    4000
}

