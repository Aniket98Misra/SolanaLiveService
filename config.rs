use std::env;

/// Runtime configuration, loaded entirely from environment variables so the
/// same binary runs unchanged locally and inside a GitHub Actions workflow.
pub struct Config {
    /// HTTP JSON-RPC endpoint. The public cluster endpoint works fine for
    /// this poll-based access pattern (a couple of requests every few
    /// minutes) — no paid RPC provider required.
    pub rpc_http_url: String,
    /// The account to watch: a wallet address or a specific token account.
    pub watch_address: String,
    /// Discord webhook URL to post alerts to.
    pub discord_webhook_url: String,
    /// Minimum SOL amount (UI units, not lamports) for a native transfer to trigger an alert.
    pub sol_threshold: f64,
    /// Minimum token amount (UI units) for an SPL `transferChecked` transfer to trigger an alert.
    pub token_threshold: f64,
    /// Max signatures to pull per poll. Keeps a single run bounded even if
    /// the address has been unexpectedly busy since the last poll.
    pub poll_limit: usize,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let rpc_http_url = required_env("RPC_HTTP_URL")?;
        let watch_address = required_env("WATCH_ADDRESS")?;
        let discord_webhook_url = required_env("DISCORD_WEBHOOK_URL")?;

        let sol_threshold = optional_env_f64("SOL_THRESHOLD", 100.0)?;
        let token_threshold = optional_env_f64("TOKEN_THRESHOLD", 10_000.0)?;
        let poll_limit = optional_env_usize("POLL_LIMIT", 25)?;

        Ok(Config {
            rpc_http_url,
            watch_address,
            discord_webhook_url,
            sol_threshold,
            token_threshold,
            poll_limit,
        })
    }
}

fn required_env(key: &str) -> Result<String, String> {
    env::var(key).map_err(|_| format!("missing required environment variable: {key}"))
}

fn optional_env_f64(key: &str, default: f64) -> Result<f64, String> {
    match env::var(key) {
        Ok(val) => val
            .parse::<f64>()
            .map_err(|_| format!("{key} must be a number, got: {val}")),
        Err(_) => Ok(default),
    }
}

fn optional_env_usize(key: &str, default: usize) -> Result<usize, String> {
    match env::var(key) {
        Ok(val) => val
            .parse::<usize>()
            .map_err(|_| format!("{key} must be a positive integer, got: {val}")),
        Err(_) => Ok(default),
    }
}
