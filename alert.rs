use crate::decode::{Asset, TransferEvent};
use serde_json::json;
use std::error::Error;

pub fn send_discord_alert(
    http: &reqwest::blocking::Client,
    webhook_url: &str,
    watch_address: &str,
    event: &TransferEvent,
) -> Result<(), Box<dyn Error>> {
    let asset_label = match &event.asset {
        Asset::Sol => "SOL".to_string(),
        Asset::Token { mint } => format!("token `{}`", short(mint)),
    };

    let direction = if event.destination == watch_address {
        "inbound to"
    } else {
        "outbound from"
    };

    let content = format!(
        "🚨 **Large transfer detected**\n\
        {amount:.4} {asset} — {direction} `{watched}`\n\
        `{source}` → `{destination}`\n\
        <https://solscan.io/tx/{sig}>",
        amount = event.ui_amount,
        asset = asset_label,
        direction = direction,
        watched = short(watch_address),
        source = short(&event.source),
        destination = short(&event.destination),
        sig = event.signature,
    );

    let response = http
        .post(webhook_url)
        .json(&json!({ "content": content }))
        .send()?;

    if !response.status().is_success() {
        return Err(format!(
            "Discord webhook returned {}: {}",
            response.status(),
            response.text().unwrap_or_default()
        )
        .into());
    }

    Ok(())
}

/// Shortens a base58 address to `first6..last4` for readable alert text.
fn short(address: &str) -> String {
    if address.len() <= 12 {
        return address.to_string();
    }
    format!("{}..{}", &address[..6], &address[address.len() - 4..])
}
