use super::types::LockfileData;
use anyhow::{Context, Result};
use std::path::PathBuf;

fn lockfile_path() -> PathBuf {
    let local_app_data = std::env::var("LOCALAPPDATA").unwrap_or_default();
    PathBuf::from(local_app_data)
        .join("Riot Games")
        .join("Riot Client")
        .join("Config")
        .join("lockfile")
}

pub fn read_lockfile() -> Result<LockfileData> {
    let path = lockfile_path();

    // The lockfile may be locked by the Riot Client, so we read via a share-mode open
    let contents = read_locked_file(&path)
        .with_context(|| format!("Failed to read lockfile at {}", path.display()))?;

    parse_lockfile(&contents)
}

fn read_locked_file(path: &std::path::Path) -> Result<String> {
    use std::fs::OpenOptions;
    use std::io::Read;

    let mut file = OpenOptions::new()
        .read(true)
        .write(false)
        .open(path)
        .with_context(|| "Riot Client not running or lockfile not found")?;

    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(contents)
}

fn parse_lockfile(contents: &str) -> Result<LockfileData> {
    let parts: Vec<&str> = contents.trim().split(':').collect();
    anyhow::ensure!(parts.len() >= 5, "Invalid lockfile format");

    Ok(LockfileData {
        port: parts[2].parse().context("Invalid port in lockfile")?,
        password: parts[3].to_string(),
        protocol: parts[4].to_string(),
    })
}

pub fn wait_for_lockfile() -> LockfileData {
    tracing::info!("Waiting for Riot Client...");
    loop {
        match read_lockfile() {
            Ok(data) => {
                tracing::info!("Lockfile found on port {}", data.port);
                return data;
            }
            Err(_) => std::thread::sleep(std::time::Duration::from_secs(2)),
        }
    }
}
