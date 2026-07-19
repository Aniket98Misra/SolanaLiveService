mod alert;
mod config;
mod decode;
mod rpc;
mod state;

use config::Config;
use decode::{decode_transfers, Asset, TransferEvent};
use rpc::RpcClient;
use std::path::Path;
use std::process::ExitCode;

const STATE_PATH: &str = "state/last_seen_signature.json";

fn main() -> ExitCode {
    let config = match Config::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("config error: {e}");
            return ExitCode::FAILURE;
        }
    };

    match run(&config) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("run failed: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let rpc = RpcClient::new(config.rpc_http_url.clone());
    let http = reqwest::blocking::Client::new();
    let state_path = Path::new(STATE_PATH);
    let mut current_state = state::load(state_path);

    let is_first_run = current_state.last_signature.is_none();

    let signatures = rpc.get_signatures_for_address(
        &config.watch_address,
        current_state.last_signature.as_deref(),
        config.poll_limit,
    )?;

    if signatures.is_empty() {
        println!("no new signatures for {}", config.watch_address);
        return Ok(());
    }

    // getSignaturesForAddress returns newest-first; the first entry is the
    // new high-water mark regardless of what we do with the rest below.
    let newest_signature = signatures[0].signature.clone();

    if is_first_run {
        // Don't replay a wallet's entire history as alerts on the very
        // first run — just establish the baseline and start watching
        // forward from here.
        println!(
            "first run: seeding state at {} ({} historical signatures skipped)",
            newest_signature,
            signatures.len()
        );
        current_state.last_signature = Some(newest_signature);
        state::save(state_path, &current_state)?;
        return Ok(());
    }

    println!("{} new signature(s) since last poll", signatures.len());

    // Process oldest-to-newest so alerts arrive in chronological order.
    for sig_info in signatures.iter().rev() {
        if sig_info.err.is_some() {
            continue; // failed transactions don't move real value
        }

        let tx = match rpc.get_transaction(&sig_info.signature) {
            Ok(Some(tx)) => tx,
            Ok(None) => {
                eprintln!("no transaction data for {}", sig_info.signature);
                continue;
            }
            Err(e) => {
                // One bad lookup shouldn't sink the whole poll — log and
                // keep going so the rest of the batch still gets checked.
                eprintln!("failed to fetch {}: {e}", sig_info.signature);
                continue;
            }
        };

        for event in decode_transfers(&tx, &sig_info.signature) {
            if !involves_watched_address(&event, &config.watch_address) {
                continue;
            }
            if !exceeds_threshold(&event, config) {
                continue;
            }

            if let Err(e) = alert::send_discord_alert(&http, &config.discord_webhook_url, &config.watch_address, &event) {
                eprintln!("failed to send alert for {}: {e}", event.signature);
            } else {
                println!("alerted on {} ({:.4} {})", event.signature, event.ui_amount, asset_name(&event.asset));
            }
        }
    }

    current_state.last_signature = Some(newest_signature);
    state::save(state_path, &current_state)?;

    Ok(())
}

fn involves_watched_address(event: &TransferEvent, watch_address: &str) -> bool {
    event.source == watch_address || event.destination == watch_address
}

fn exceeds_threshold(event: &TransferEvent, config: &Config) -> bool {
    match &event.asset {
        Asset::Sol => event.ui_amount >= config.sol_threshold,
        Asset::Token { .. } => event.ui_amount >= config.token_threshold,
    }
}

fn asset_name(asset: &Asset) -> String {
    match asset {
        Asset::Sol => "SOL".to_string(),
        Asset::Token { mint } => format!("token({mint})"),
    }
}
