use super::types::{EntitlementsResponse, LockfileData, RiotAuth};
use anyhow::{Context, Result};
use base64::Engine;

pub async fn authenticate(lockfile: &LockfileData) -> Result<RiotAuth> {
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()?;

    let basic_auth =
        base64::engine::general_purpose::STANDARD.encode(format!("riot:{}", lockfile.password));

    let url = format!("https://127.0.0.1:{}/entitlements/v1/token", lockfile.port);

    let resp: EntitlementsResponse = client
        .get(&url)
        .header("Authorization", format!("Basic {}", basic_auth))
        .send()
        .await
        .context("Failed to connect to Riot Client")?
        .json()
        .await
        .context("Failed to parse entitlements")?;

    let (region, shard) = detect_region_shard().unwrap_or(("na".into(), "na".into()));

    Ok(RiotAuth {
        puuid: resp.subject.clone(),
        lockfile: lockfile.clone(),
        access_token: resp.access_token,
        entitlements_token: resp.token,
        region,
        shard,
    })
}

fn detect_region_shard() -> Option<(String, String)> {
    let local_app_data = std::env::var("LOCALAPPDATA").ok()?;
    let log_path = std::path::PathBuf::from(&local_app_data)
        .join("VALORANT")
        .join("Saved")
        .join("Logs")
        .join("ShooterGame.log");

    let contents = std::fs::read_to_string(&log_path).ok()?;

    let mut region = None;
    let mut shard = None;

    for line in contents.lines().rev() {
        if region.is_none() {
            if let Some(idx) = line.find("https://glz-") {
                let after = &line[idx + 12..];
                if let Some(dash) = after.find('-') {
                    region = Some(after[..dash].to_string());
                }
            }
        }
        if shard.is_none() {
            if let Some(idx) = line.find("https://pd.") {
                let after = &line[idx + 11..];
                if let Some(dot) = after.find('.') {
                    shard = Some(after[..dot].to_string());
                }
            }
        }
        if region.is_some() && shard.is_some() {
            break;
        }
    }

    match (region, shard) {
        (Some(r), Some(s)) => Some((r, s)),
        _ => None,
    }
}
