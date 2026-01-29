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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // RunnerConfig tests
    #[test]
    fn test_runner_config_default() {
        let config = RunnerConfig::default();
        assert_eq!(config.parallel, 0);
        assert!(config.extra_args.is_empty());
    }

    #[test]
    fn test_runner_config_debug() {
        let config = RunnerConfig::default();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("parallel"));
        assert!(debug_str.contains("extra_args"));
    }

    // WatchConfig tests
    #[test]
    fn test_watch_config_default() {
        let config = WatchConfig::default();
        assert_eq!(config.debounce_ms, 500);
        assert_eq!(config.patterns, vec!["*.cs", "*.csproj"]);
        assert_eq!(config.ignore, vec!["**/obj/**", "**/bin/**"]);
    }

    #[test]
    fn test_watch_config_debug() {
        let config = WatchConfig::default();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("debounce_ms"));
        assert!(debug_str.contains("patterns"));
        assert!(debug_str.contains("ignore"));
    }

    // ThemeConfig tests
    #[test]
    fn test_theme_config_default() {
        let config = ThemeConfig::default();
        assert!(config.name.is_none());
    }

    #[test]
    fn test_theme_config_debug() {
        let config = ThemeConfig::default();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("name"));
    }

    // Config tests
    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.runner.parallel, 0);
        assert_eq!(config.watch.debounce_ms, 500);
        assert!(config.theme.name.is_none());
    }

    #[test]
    fn test_config_load_no_file() {
        let temp_dir = TempDir::new().unwrap();
        let config = Config::load(temp_dir.path());

        // Should return defaults
        assert_eq!(config.runner.parallel, 0);
        assert_eq!(config.watch.debounce_ms, 500);
        assert!(config.theme.name.is_none());
    }

    #[test]
    fn test_config_load_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join(".testament.toml");
        fs::write(&config_path, "").unwrap();

        let config = Config::load(temp_dir.path());

        // Empty file should parse as defaults
        assert_eq!(config.runner.parallel, 0);
        assert_eq!(config.watch.debounce_ms, 500);
    }

    #[test]
    fn test_config_load_runner_section() {
        let temp_dir = TempDir::new().unwrap();
        let config_content = r#"
[runner]
parallel = 4
extra_args = ["--no-build", "--verbosity", "quiet"]
"#;
        let config_path = temp_dir.path().join(".testament.toml");
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(temp_dir.path());

        assert_eq!(config.runner.parallel, 4);
        assert_eq!(config.runner.extra_args, vec!["--no-build", "--verbosity", "quiet"]);
    }

    #[test]
    fn test_config_load_watch_section() {
        let temp_dir = TempDir::new().unwrap();
        let config_content = r#"
[watch]
debounce_ms = 1000
patterns = ["*.cs", "*.fs"]
ignore = ["**/bin/**"]
"#;
        let config_path = temp_dir.path().join(".testament.toml");
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(temp_dir.path());

        assert_eq!(config.watch.debounce_ms, 1000);
        assert_eq!(config.watch.patterns, vec!["*.cs", "*.fs"]);
        assert_eq!(config.watch.ignore, vec!["**/bin/**"]);
    }

    #[test]
    fn test_config_load_theme_section() {
        let temp_dir = TempDir::new().unwrap();
        let config_content = r#"
[theme]
name = "dark"
"#;
        let config_path = temp_dir.path().join(".testament.toml");
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(temp_dir.path());

        assert_eq!(config.theme.name, Some("dark".to_string()));
    }

    #[test]
    fn test_config_load_full_config() {
        let temp_dir = TempDir::new().unwrap();
        let config_content = r#"
[runner]
parallel = 8
extra_args = ["--filter", "Category=Unit"]

[watch]
debounce_ms = 250
patterns = ["*.cs"]
ignore = ["**/obj/**", "**/bin/**", "**/node_modules/**"]

[theme]
name = "amber"
"#;
        let config_path = temp_dir.path().join(".testament.toml");
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(temp_dir.path());

        assert_eq!(config.runner.parallel, 8);
        assert_eq!(config.runner.extra_args, vec!["--filter", "Category=Unit"]);
        assert_eq!(config.watch.debounce_ms, 250);
        assert_eq!(config.watch.patterns, vec!["*.cs"]);
        assert_eq!(config.watch.ignore, vec!["**/obj/**", "**/bin/**", "**/node_modules/**"]);
        assert_eq!(config.theme.name, Some("amber".to_string()));
    }

    #[test]
    fn test_config_load_partial_sections() {
        let temp_dir = TempDir::new().unwrap();
        let config_content = r#"
[runner]
parallel = 2
"#;
        let config_path = temp_dir.path().join(".testament.toml");
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(temp_dir.path());

        // Runner is customized
        assert_eq!(config.runner.parallel, 2);
        assert!(config.runner.extra_args.is_empty());

        // Watch and theme use defaults
        assert_eq!(config.watch.debounce_ms, 500);
        assert!(config.theme.name.is_none());
    }

    #[test]
    fn test_config_load_malformed_toml_returns_default() {
        let temp_dir = TempDir::new().unwrap();
        let config_content = r#"
[runner
parallel = broken
"#;
        let config_path = temp_dir.path().join(".testament.toml");
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(temp_dir.path());

        // Should return defaults when parsing fails
        assert_eq!(config.runner.parallel, 0);
        assert_eq!(config.watch.debounce_ms, 500);
    }

    #[test]
    fn test_config_load_wrong_types_returns_default() {
        let temp_dir = TempDir::new().unwrap();
        let config_content = r#"
[runner]
parallel = "not a number"
"#;
        let config_path = temp_dir.path().join(".testament.toml");
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(temp_dir.path());

        // Should return defaults when parsing fails
        assert_eq!(config.runner.parallel, 0);
    }

    #[test]
    fn test_config_load_unknown_fields_ignored() {
        let temp_dir = TempDir::new().unwrap();
        let config_content = r#"
[runner]
parallel = 4
unknown_field = "value"

[unknown_section]
some_key = "some_value"
"#;
        let config_path = temp_dir.path().join(".testament.toml");
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(temp_dir.path());

        // Known fields should still be parsed
        assert_eq!(config.runner.parallel, 4);
    }

    #[test]
    fn test_config_debug() {
        let config = Config::default();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("runner"));
        assert!(debug_str.contains("watch"));
        assert!(debug_str.contains("theme"));
    }

    #[test]
    fn test_default_parallel_value() {
        assert_eq!(default_parallel(), 0);
    }

    #[test]
    fn test_default_debounce_value() {
        assert_eq!(default_debounce(), 500);
    }
}
