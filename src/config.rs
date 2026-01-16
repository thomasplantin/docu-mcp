use std::path::PathBuf;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use dirs;

/// Configuration structure for the MCP server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// List of directories to monitor
    pub directories: Vec<String>,
    /// Currently active directory
    pub active_directory: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            directories: Vec::new(),
            active_directory: None,
        }
    }
}

/// Get the platform-specific configuration file path
///
/// # Returns
/// * Unix/macOS: `~/.config/docu-mcp/config.json`
/// * Windows: `%APPDATA%\docu-mcp\config.json`
pub fn get_config_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("Failed to get config directory"))?;
    
    let mut config_path = config_dir;
    config_path.push("docu-mcp");
    config_path.push("config.json");
    
    Ok(config_path)
}

/// Load configuration from file, creating default if missing
///
/// # Returns
/// * `Ok(Config)` - Loaded or default configuration
/// * `Err` - Error if file exists but cannot be read/parsed
pub fn load_config() -> Result<Config> {
    let config_path = get_config_path()?;
    
    // If config file doesn't exist, return default
    if !config_path.exists() {
        return Ok(Config::default());
    }
    
    // Read and parse config file
    let content = std::fs::read_to_string(&config_path)
        .with_context(|| format!("Failed to read config file: {}", config_path.display()))?;
    
    let config: Config = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse config file: {}", config_path.display()))?;
    
    Ok(config)
}

/// Save configuration to file
///
/// # Arguments
/// * `config` - Configuration to save
///
/// # Returns
/// * `Ok(())` - Success
/// * `Err` - Error if file cannot be written
pub fn save_config(config: &Config) -> Result<()> {
    let config_path = get_config_path()?;
    
    // Create parent directories if they don't exist
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create config directory: {}", parent.display()))?;
    }
    
    // Serialize and write config
    let content = serde_json::to_string_pretty(config)
        .context("Failed to serialize config")?;
    
    std::fs::write(&config_path, content)
        .with_context(|| format!("Failed to write config file: {}", config_path.display()))?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert!(config.directories.is_empty());
        assert!(config.active_directory.is_none());
    }

    #[test]
    fn test_config_serialization() {
        let mut config = Config::default();
        config.directories.push("/test/path".to_string());
        config.active_directory = Some("/test/path".to_string());

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: Config = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.directories.len(), 1);
        assert_eq!(deserialized.directories[0], "/test/path");
        assert_eq!(deserialized.active_directory, Some("/test/path".to_string()));
    }
}
