use crate::app::AppState;
use anyhow::{Context, Result};
use semver::Version;
use serde::Deserialize;
use std::process::Command;
use std::sync::Arc;
use tokio::sync::RwLock;

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const GITHUB_RELEASE_API: &str = "https://api.github.com/repos/resirch/star-client/releases/latest";

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
}

#[derive(Debug)]
struct AvailableUpdate {
    version: Version,
    html_url: String,
}

pub async fn maybe_prompt_for_update(app_state: &Arc<RwLock<AppState>>) -> Result<()> {
    let auto_check_updates = {
        let state = app_state.read().await;
        state.config.behavior.auto_check_updates
    };

    if !auto_check_updates {
        return Ok(());
    }

    let Some(update) = check_for_update().await? else {
        return Ok(());
    };

    tracing::info!(
        "Update available: current=v{}, latest=v{}",
        CURRENT_VERSION,
        update.version
    );

    if prompt_for_update(&update)? {
        open_release_page(&update.html_url)?;
    }

    Ok(())
}

async fn check_for_update() -> Result<Option<AvailableUpdate>> {
    let client = reqwest::Client::builder()
        .user_agent(format!("star-client/{}", CURRENT_VERSION))
        .build()
        .context("failed to build updater HTTP client")?;

    let release = client
        .get(GITHUB_RELEASE_API)
        .send()
        .await
        .context("failed to query GitHub releases API")?
        .error_for_status()
        .context("GitHub releases API returned an error")?
        .json::<GitHubRelease>()
        .await
        .context("failed to parse GitHub release response")?;

    let Some(version) = newer_release_version(&release.tag_name, CURRENT_VERSION)? else {
        return Ok(None);
    };

    Ok(Some(AvailableUpdate {
        version,
        html_url: release.html_url,
    }))
}

fn newer_release_version(tag_name: &str, current_version: &str) -> Result<Option<Version>> {
    let release_version = parse_version_tag(tag_name)?;
    let current_version =
        Version::parse(current_version).context("current app version is not valid semver")?;

    if release_version > current_version {
        Ok(Some(release_version))
    } else {
        Ok(None)
    }
}

fn parse_version_tag(tag_name: &str) -> Result<Version> {
    let normalized = tag_name
        .trim()
        .trim_start_matches(|ch| ch == 'v' || ch == 'V');
    Version::parse(normalized)
        .with_context(|| format!("release tag '{tag_name}' is not valid semver"))
}

#[cfg(target_os = "windows")]
fn prompt_for_update(update: &AvailableUpdate) -> Result<bool> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, IDYES, MB_ICONQUESTION, MB_SETFOREGROUND, MB_TOPMOST, MB_YESNO,
    };

    let title = utf16_null("Star Client Update");
    let message = utf16_null(&format!(
        "Star Client v{} is available.\nYou are running v{}.\n\nOpen the GitHub release page to update?",
        update.version, CURRENT_VERSION
    ));

    let choice = unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            message.as_ptr(),
            title.as_ptr(),
            MB_YESNO | MB_ICONQUESTION | MB_SETFOREGROUND | MB_TOPMOST,
        )
    };

    Ok(choice == IDYES)
}

#[cfg(not(target_os = "windows"))]
fn prompt_for_update(update: &AvailableUpdate) -> Result<bool> {
    tracing::info!(
        "Update available: current=v{}, latest=v{}, url={}",
        CURRENT_VERSION,
        update.version,
        update.html_url
    );
    Ok(false)
}

fn open_release_page(url: &str) -> Result<()> {
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("explorer");
        command.arg(url);
        command
    };

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg(url);
        command
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(url);
        command
    };

    command
        .spawn()
        .with_context(|| format!("failed to open release page: {url}"))?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn utf16_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::{newer_release_version, parse_version_tag};

    #[test]
    fn parses_v_prefixed_release_tags() {
        let version = parse_version_tag("v1.0.1").unwrap();
        assert_eq!(version.to_string(), "1.0.1");
    }

    #[test]
    fn detects_newer_release_versions() {
        let version = newer_release_version("v1.0.2", "1.0.1").unwrap();
        assert_eq!(version.unwrap().to_string(), "1.0.2");
    }

    #[test]
    fn ignores_equal_or_older_release_versions() {
        assert!(newer_release_version("v1.0.1", "1.0.1").unwrap().is_none());
        assert!(newer_release_version("v1.0.0", "1.0.1").unwrap().is_none());
    }
}
