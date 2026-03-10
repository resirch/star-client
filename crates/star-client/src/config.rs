use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub overlay: OverlayConfig,
    #[serde(default)]
    pub columns: ColumnConfig,
    #[serde(default)]
    pub behavior: BehaviorConfig,
    #[serde(default)]
    pub star: StarConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlayConfig {
    #[serde(default = "default_hotkey")]
    pub hotkey: String,
    #[serde(default = "default_weapon")]
    pub weapon: String,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnConfig {
    #[serde(default = "bool_true")]
    pub skin: bool,
    #[serde(default = "bool_true")]
    pub rr: bool,
    #[serde(default = "bool_true")]
    pub earned_rr: bool,
    #[serde(default = "bool_true")]
    pub peak_rank: bool,
    #[serde(default = "bool_true")]
    pub previous_rank: bool,
    #[serde(default = "bool_true")]
    pub leaderboard: bool,
    #[serde(default = "bool_true")]
    pub headshot_percent: bool,
    #[serde(default = "bool_true")]
    pub winrate: bool,
    #[serde(default = "bool_true")]
    pub kd: bool,
    #[serde(default = "bool_true")]
    pub level: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorConfig {
    #[serde(default = "bool_true")]
    pub auto_show_pregame: bool,
    #[serde(default = "bool_true")]
    pub auto_hide_ingame: bool,
    #[serde(default = "bool_true")]
    pub respect_streamer_mode: bool,
    #[serde(default = "bool_true")]
    pub party_finder: bool,
    #[serde(default = "bool_true")]
    pub discord_rpc: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StarConfig {
    #[serde(default = "bool_true")]
    pub enabled: bool,
    #[serde(default = "default_backend_url")]
    pub backend_url: String,
}

fn bool_true() -> bool {
    true
}
fn default_hotkey() -> String {
    "F2".into()
}
fn default_weapon() -> String {
    "Vandal".into()
}
fn default_opacity() -> f32 {
    0.85
}
fn default_backend_url() -> String {
    "https://star-api.fly.dev".into()
}

impl Default for Config {
    fn default() -> Self {
        toml::from_str(include_str!("../../../config.default.toml"))
            .expect("default config must parse")
    }
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            hotkey: default_hotkey(),
            weapon: default_weapon(),
            opacity: default_opacity(),
        }
    }
}

impl Default for ColumnConfig {
    fn default() -> Self {
        Self {
            skin: true,
            rr: true,
            earned_rr: true,
            peak_rank: true,
            previous_rank: true,
            leaderboard: true,
            headshot_percent: true,
            winrate: true,
            kd: true,
            level: true,
        }
    }
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            auto_show_pregame: true,
            auto_hide_ingame: true,
            respect_streamer_mode: true,
            party_finder: true,
            discord_rpc: true,
        }
    }
}

impl Default for StarConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            backend_url: default_backend_url(),
        }
    }
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let path = Self::config_path();
        if path.exists() {
            let contents = std::fs::read_to_string(&path)?;
            let config: Config = toml::from_str(&contents)?;
            Ok(config)
        } else {
            let config = Config::default();
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, toml::to_string_pretty(&config)?)?;
            Ok(config)
        }
    }

    pub fn config_path() -> PathBuf {
        let dirs =
            directories::ProjectDirs::from("dev", "star", "star-client").expect("home directory");
        dirs.config_dir().join("config.toml")
    }

    pub fn data_dir() -> PathBuf {
        let dirs =
            directories::ProjectDirs::from("dev", "star", "star-client").expect("home directory");
        dirs.data_dir().to_path_buf()
    }
}
