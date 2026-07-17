use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum Asset {
    Sol,
    Token { mint: String },
}

#[derive(Debug, Clone)]
pub struct TransferEvent {
    pub signature: String,
    pub asset: Asset,
    pub source: String,
    pub destination: String,
    pub ui_amount: f64,
}

/// Walks a `getTransaction` (jsonParsed) response and extracts every
/// top-level transfer instruction (native SOL via the System Program, or
/// SPL tokens via `transferChecked`).
///
/// Two things this deliberately does *not* do, documented here rather than
/// discovered later:
///
/// - It only looks at top-level instructions, not `innerInstructions`. A
///   transfer routed through a CPI call (a swap, for instance) won't be
///   caught. Catching those needs a second pass over `meta.innerInstructions`
///   with the same parsing logic — a reasonable follow-up, not included here
///   to keep the decode path easy to reason about.
/// - It only decodes `transferChecked`, not the legacy `transfer` SPL
///   instruction. `transferChecked` carries the token's decimals directly
///   in the response so the amount can be converted to UI units without a
///   second RPC call for mint metadata; plain `transfer` doesn't, and most
///   wallets/programs have used `transferChecked` for years, so the
///   coverage gap in practice is small.
pub fn decode_transfers(tx: &Value, signature: &str) -> Vec<TransferEvent> {
    let instructions = tx
        .pointer("/transaction/message/instructions")
        .and_then(Value::as_array);

    let Some(instructions) = instructions else {
        return Vec::new();
    };

    instructions
        .iter()
        .filter_map(|ix| decode_instruction(ix, signature))
        .collect()
}

fn decode_instruction(ix: &Value, signature: &str) -> Option<TransferEvent> {
    let program = ix.get("program")?.as_str()?;
    let parsed = ix.get("parsed")?;
    let kind = parsed.get("type")?.as_str()?;
    let info = parsed.get("info")?;

    match (program, kind) {
        ("system", "transfer") => {
            let lamports = info.get("lamports")?.as_u64()?;
            Some(TransferEvent {
                signature: signature.to_string(),
                asset: Asset::Sol,
                source: info.get("source")?.as_str()?.to_string(),
                destination: info.get("destination")?.as_str()?.to_string(),
                ui_amount: lamports as f64 / 1_000_000_000.0,
            })
        }
        ("spl-token", "transferChecked") => {
            let token_amount = info.get("tokenAmount")?;
            let ui_amount = token_amount.get("uiAmount")?.as_f64()?;
            let mint = info.get("mint")?.as_str()?.to_string();
            Some(TransferEvent {
                signature: signature.to_string(),
                asset: Asset::Token { mint },
                source: info.get("source")?.as_str()?.to_string(),
                destination: info.get("destination")?.as_str()?.to_string(),
                ui_amount,
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tx_with_instructions(instructions: Value) -> Value {
        json!({
            "transaction": {
                "message": {
                    "instructions": instructions
                }
            }
        })
    }

    #[test]
    fn decodes_native_sol_transfer() {
        let tx = tx_with_instructions(json!([
            {
                "program": "system",
                "parsed": {
                    "type": "transfer",
                    "info": {
                        "source": "SourceWallet111111111111111111111111111",
                        "destination": "DestWallet1111111111111111111111111111",
                        "lamports": 250_000_000_000u64
                    }
                }
            }
        ]));

        let events = decode_transfers(&tx, "sig123");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].asset, Asset::Sol);
        assert_eq!(events[0].ui_amount, 250.0);
    }

    #[test]
    fn decodes_spl_transfer_checked() {
        let tx = tx_with_instructions(json!([
            {
                "program": "spl-token",
                "parsed": {
                    "type": "transferChecked",
                    "info": {
                        "source": "SourceTokenAccount11111111111111111111",
                        "destination": "DestTokenAccount111111111111111111111",
                        "mint": "MintAddress1111111111111111111111111111",
                        "tokenAmount": {
                            "amount": "15000000000",
                            "decimals": 6,
                            "uiAmount": 15000.0,
                            "uiAmountString": "15000"
                        }
                    }
                }
            }
        ]));

        let events = decode_transfers(&tx, "sig456");
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].asset,
            Asset::Token {
                mint: "MintAddress1111111111111111111111111111".to_string()
            }
        );
        assert_eq!(events[0].ui_amount, 15000.0);
    }

    #[test]
    fn ignores_unrelated_instructions() {
        let tx = tx_with_instructions(json!([
            {
                "program": "spl-token",
                "parsed": {
                    "type": "approve",
                    "info": { "amount": "1" }
                }
            },
            {
                "programId": "SomeUnparsedProgram11111111111111111111"
            }
        ]));

        let events = decode_transfers(&tx, "sig789");
        assert!(events.is_empty());
    }

    #[test]
    fn missing_instructions_field_returns_empty() {
        let tx = json!({ "transaction": { "message": {} } });
        let events = decode_transfers(&tx, "sigabc");
        assert!(events.is_empty());
    }
}
