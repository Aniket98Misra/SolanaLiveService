use serde::Deserialize;
use serde_json::{json, Value};
use std::error::Error;

/// One entry from `getSignaturesForAddress`.
#[derive(Debug, Deserialize)]
pub struct SignatureInfo {
    pub signature: String,
    /// `Some(_)` means the transaction failed on-chain; we skip those.
    pub err: Option<Value>,
}

/// Thin wrapper around a single Solana JSON-RPC HTTP endpoint.
///
/// This talks to the RPC directly over HTTP with hand-built request bodies
/// rather than pulling in the full `solana-client` / `solana-sdk` crate
/// tree. For a script that makes two kinds of read-only calls, that's a
/// lot of dependency weight for not much benefit — and being explicit
/// about the request/response shape makes it obvious exactly what's being
/// asked of the RPC node, which matters for a project meant to explain
/// itself.
pub struct RpcClient {
    http: reqwest::blocking::Client,
    url: String,
}

impl RpcClient {
    pub fn new(url: String) -> Self {
        Self {
            http: reqwest::blocking::Client::new(),
            url,
        }
    }

    fn call(&self, method: &str, params: Value) -> Result<Value, Box<dyn Error>> {
        let body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        });

        let response: Value = self
            .http
            .post(&self.url)
            .json(&body)
            .send()?
            .json()?;

        if let Some(err) = response.get("error") {
            return Err(format!("RPC error calling {method}: {err}").into());
        }

        response
            .get("result")
            .cloned()
            .ok_or_else(|| format!("RPC response for {method} had no result field: {response}").into())
    }

    /// Returns signatures for transactions that touched `address`, newest first.
    /// If `until` is `Some(sig)`, only signatures more recent than `sig` are returned —
    /// this is how we avoid re-processing transactions across runs.
    pub fn get_signatures_for_address(
        &self,
        address: &str,
        until: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SignatureInfo>, Box<dyn Error>> {
        let mut opts = json!({ "limit": limit });
        if let Some(sig) = until {
            opts["until"] = json!(sig);
        }

        let result = self.call("getSignaturesForAddress", json!([address, opts]))?;
        let sigs: Vec<SignatureInfo> = serde_json::from_value(result)?;
        Ok(sigs)
    }

    /// Fetches a single transaction with parsed instruction data, so SPL Token
    /// and System Program instructions arrive as structured JSON (`parsed.info`)
    /// instead of raw base58 instruction data we'd have to decode ourselves.
    pub fn get_transaction(&self, signature: &str) -> Result<Option<Value>, Box<dyn Error>> {
        let params = json!([
            signature,
            { "encoding": "jsonParsed", "maxSupportedTransactionVersion": 0 }
        ]);
        let result = self.call("getTransaction", params)?;
        if result.is_null() {
            return Ok(None);
        }
        Ok(Some(result))
    }
}
