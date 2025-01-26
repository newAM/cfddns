use anyhow::Context as _;
use cloudflare::framework::async_api::Client;
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsString,
    fs::{File, OpenOptions},
    io::{BufReader, BufWriter},
    net::{Ipv4Addr, Ipv6Addr},
    path::{Path, PathBuf},
    str::FromStr as _,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordConfig {
    /// Record name
    pub name: String,
    /// TTL
    pub ttl: Option<u32>,
    /// Whether the record is proxied by Cloudflare
    pub proxied: Option<bool>,
    /// EUI-64 suffix for AAAA record updates
    ///
    /// AAAA records are not updated if None.
    pub eui64: Option<Ipv6Addr>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZoneConfig {
    /// Zone identifier for this domain
    pub name: String,
    /// Records for this zone
    pub records: Vec<RecordConfig>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfigFile {
    a_interface: Option<String>,
    a_http: Option<url::Url>,
    aaaa_interface: Option<String>,
    aaaa_http: Option<url::Url>,
    zones: Vec<ZoneConfig>,
    history_path: PathBuf,
    log_level: String,
}

#[derive(Deserialize, Serialize, Default, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct History {
    pub ipv4: Option<Ipv4Addr>,
    pub ipv6_prefix: Option<Ipv6Addr>,
}

pub struct Config {
    pub a_interface: Option<String>,
    pub a_http: Option<url::Url>,
    pub aaaa_interface: Option<String>,
    pub aaaa_http: Option<url::Url>,
    pub zones: Vec<ZoneConfig>,
    pub history: History,
    pub history_path: PathBuf,
    pub cloudflare_client: Client,
}

impl Config {
    pub fn from_args_os() -> anyhow::Result<Config> {
        let config_file_path: OsString = match std::env::args_os().nth(1) {
            Some(x) => x,
            None => {
                eprintln!(
                    "usage: {} [config-file.json]",
                    std::env::args_os()
                        .next()
                        .unwrap_or_else(|| OsString::from("???"))
                        .to_string_lossy()
                );
                std::process::exit(1);
            }
        };

        let file: File = File::open(&config_file_path).with_context(|| {
            format!(
                "Failed to open config file at {}",
                config_file_path.to_string_lossy()
            )
        })?;
        let reader: BufReader<File> = BufReader::new(file);
        let config: ConfigFile =
            serde_json::from_reader(reader).context("Failed to deserialize config file")?;

        let level: log::LevelFilter =
            log::LevelFilter::from_str(&config.log_level).with_context(|| {
                format!(
                    "Invalid log_level in configuration file {}",
                    config_file_path.to_string_lossy()
                )
            })?;

        if level != log::LevelFilter::Off {
            systemd_journal_logger::JournalLog::new()
                .context("Failed to create logger")?
                .install()
                .context("Failed to install logger")?;
            log::set_max_level(level);
        }

        const CLOUDFLARE_TOKEN_ENV_VAR: &str = "CLOUDFLARE_TOKEN";

        let cloudflare_token: String =
            std::env::var(CLOUDFLARE_TOKEN_ENV_VAR).with_context(|| {
                format!(
                    "Failed to read cloudflare API token from environment variable '{}'",
                    CLOUDFLARE_TOKEN_ENV_VAR
                )
            })?;

        let cloudflare_client: Client = Client::new(
            cloudflare::framework::auth::Credentials::UserAuthToken {
                token: cloudflare_token,
            },
            cloudflare::framework::HttpApiClientConfig::default(),
            cloudflare::framework::Environment::Production,
        )
        .context("Failed to create Cloudflare API client")?;

        let history: History = restore_history(&config.history_path)?;

        Ok(Config {
            a_interface: config.a_interface,
            a_http: config.a_http,
            aaaa_interface: config.aaaa_interface,
            aaaa_http: config.aaaa_http,
            zones: config.zones,
            history,
            history_path: config.history_path,
            cloudflare_client,
        })
    }
}

fn restore_history(path: &Path) -> anyhow::Result<History> {
    match File::open(path) {
        Ok(file) => {
            let reader: BufReader<File> = BufReader::new(file);

            let history: History =
                serde_json::from_reader(reader).context("Failed to deserialize history file")?;

            Ok(history)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            log::warn!(
                "History file does not exist at '{}' creating new history file",
                path.to_string_lossy()
            );

            save_history(path, History::default()).with_context(|| {
                format!(
                    "Failed to create initial history file at '{}'",
                    path.to_string_lossy()
                )
            })?;

            Ok(History::default())
        }
        Err(e) => Err(e).with_context(|| {
            format!(
                "Failed to open history file at '{}'",
                path.to_string_lossy()
            )
        }),
    }
}

pub fn save_history(path: &Path, history: History) -> anyhow::Result<()> {
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .with_context(|| {
            format!(
                "Failed to open history file at '{}' for writing",
                path.to_string_lossy()
            )
        })?;
    let writer = BufWriter::new(file);

    serde_json::to_writer(writer, &history).context("Failed to write history to file")
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::TempDir;

    #[test]
    fn restore_history_file_creation() {
        let hist_dir: TempDir = TempDir::new().unwrap();
        let mut hist_file_path = hist_dir.into_path();
        hist_file_path.push("history.json");

        assert!(!hist_file_path.exists());

        assert_eq!(
            restore_history(&hist_file_path).unwrap(),
            History::default()
        );

        assert!(hist_file_path.exists());
    }

    #[test]
    fn save_restore_history() {
        let hist_dir: TempDir = TempDir::new().unwrap();
        let mut hist_file_path = hist_dir.into_path();
        hist_file_path.push("history.json");

        // file creation
        save_history(&hist_file_path, History::default()).unwrap();

        // restore history
        let restored = restore_history(&hist_file_path).unwrap();
        assert_eq!(restored, History::default());

        const HISTORY_UNSPECIFIED_ADDR: History = History {
            ipv4: Some(Ipv4Addr::UNSPECIFIED),
            ipv6_prefix: Some(Ipv6Addr::UNSPECIFIED),
        };

        // file overwrite
        save_history(&hist_file_path, HISTORY_UNSPECIFIED_ADDR).unwrap();

        // restore overwritten history
        let restored = restore_history(&hist_file_path).unwrap();
        assert_eq!(restored, HISTORY_UNSPECIFIED_ADDR);
    }
}
