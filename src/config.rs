use std::path::Path;

use serde::Deserialize;

#[derive(Debug, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub runner: RunnerConfig,
    #[serde(default)]
    pub watch: WatchConfig,
    #[serde(default)]
    pub theme: ThemeConfig,
}

#[derive(Debug, Deserialize)]
pub struct RunnerConfig {
    #[serde(default = "default_parallel")]
    pub parallel: u32,
    #[serde(default)]
    pub extra_args: Vec<String>,
}

impl Default for RunnerConfig {
    fn default() -> Self {
        Self {
            parallel: default_parallel(),
            extra_args: Vec::new(),
        }
    }
}

fn default_parallel() -> u32 {
    0 // 0 means use dotnet's default
}

#[derive(Debug, Deserialize)]
pub struct WatchConfig {
    #[serde(default = "default_debounce")]
    pub debounce_ms: u64,
    #[serde(default)]
    pub patterns: Vec<String>,
    #[serde(default)]
    pub ignore: Vec<String>,
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            debounce_ms: default_debounce(),
            patterns: vec!["*.cs".to_string(), "*.csproj".to_string()],
            ignore: vec!["**/obj/**".to_string(), "**/bin/**".to_string()],
        }
    }
}

fn default_debounce() -> u64 {
    500
}

#[derive(Debug, Deserialize, Default)]
pub struct ThemeConfig {
    #[serde(default)]
    pub name: Option<String>,
}

impl Config {
    pub fn load(dir: &Path) -> Self {
        let config_path = dir.join(".testament.toml");
        if config_path.exists() {
            match std::fs::read_to_string(&config_path) {
                Ok(content) => match toml::from_str(&content) {
                    Ok(config) => return config,
                    Err(e) => {
                        eprintln!("Warning: Failed to parse .testament.toml: {}", e);
                    }
                },
                Err(e) => {
                    eprintln!("Warning: Failed to read .testament.toml: {}", e);
                }
            }
        }
        Config::default()
    }
}
