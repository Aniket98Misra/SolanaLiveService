# solana-transfer-watch

A small Rust bot that watches a Solana wallet or token account for unusually
large SOL or SPL token transfers, and posts an alert to Discord when one
crosses a threshold you set.

It runs as a scheduled GitHub Action, polling every ~10 minutes. There is no
server, no database, and no paid infrastructure anywhere in this project —
that's not a limitation I'm apologizing for, it's the design. More on why
below.

```
🚨 Large transfer detected
15000.0000 token `MintAd..1111` — inbound to `Watche..2222`
SourceT..1111 → DestTo..2222
https://solscan.io/tx/5h3k...
```

---

## What it actually watches

Point it at any Solana address — a wallet, or a specific token account —
via `WATCH_ADDRESS`. Every poll, it:

1. Asks the RPC node for signatures involving that address since the last
   time it checked (`getSignaturesForAddress`, using the `until` cursor).
2. Fetches each new transaction with `jsonParsed` encoding, which gives
   instruction data back as structured JSON instead of raw base58 bytes.
3. Picks out **native SOL transfers** (System Program `transfer`) and
   **SPL token transfers** (`transferChecked`), converts the amount to UI
   units, and checks it against your threshold.
4. Posts to a Discord webhook for anything that clears the bar.
5. Writes the newest signature it saw back to `state/last_seen_signature.json`
   and exits.

That's it — no long-running process, no in-memory state, no websocket to
babysit. Every run is a clean, stateless-except-for-one-file script.

## Why this is Solana-specific, not "any-chain-bot-with-a-URL-changed"

The part that matters isn't "poll an RPC and check a number" — that's
chain-agnostic plumbing. It's in `src/decode.rs`: SPL token transfers don't
carry a human-readable amount in the transaction logs. The amount lives in
the instruction data, and `transferChecked` — the instruction modern
wallets and programs actually use — carries the mint's decimals alongside
the raw amount specifically so a client doesn't have to go fetch mint
metadata separately just to know if `15000000000` means 15,000 tokens or
15. Decoding that correctly, and knowing *why* `transferChecked` exists
instead of the older `transfer` instruction (it added the mint and decimals
as an explicit safety check against sending the wrong token entirely), is
Solana/SPL-token knowledge, not generic web3 boilerplate.

## Architecture

```
                    ┌─────────────────────────┐
  GitHub Actions    │   cron: */10 * * * *     │
  (no server)       └────────────┬─────────────┘
                                  │ runs
                                  ▼
                     ┌────────────────────────┐
                     │  cargo run (one shot)   │
                     └────────────┬─────────────┘
                                  │
       state/last_seen_signature.json ──▶ load cursor
                                  │
                                  ▼
              getSignaturesForAddress(address, until=cursor)
                                  │
                     for each new, non-failed signature
                                  ▼
                        getTransaction(sig, jsonParsed)
                                  │
                     decode SOL / SPL transferChecked
                                  │
                       amount ≥ threshold? ──▶ Discord webhook
                                  │
                                  ▼
                 write newest signature ──▶ git commit + push
```

## Why polling instead of a websocket subscription

The more "obvious" version of this bot subscribes to `logsSubscribe` over a
websocket and reacts in real time. I built it that way first, then deployed
it, and ran into the actual constraint: a websocket subscription needs a
*process that stays alive*, and every genuinely free hosting option left in
2026 either sleeps your service after 15 minutes of inactivity (killing the
socket) or asks for a credit card at signup, even for tiers that never
charge you.

Polling sidesteps the problem instead of solving it: a scheduled GitHub
Action *is* the runtime. Public repos get free Actions minutes, no card,
no third-party account, no VM to patch or forget about. The trade-off is
honest and worth stating plainly — you get near-real-time (minutes) instead
of real-time (seconds). For a wallet-monitoring alert, that's a fine trade.
For a trading bot reacting to price movements, it wouldn't be — this
project is the former, deliberately.

## Setup

**1. RPC endpoint.** The public cluster endpoint
(`https://api.mainnet-beta.solana.com`) works fine for this — a poll every
10 minutes is a couple of requests, well under public rate limits. No RPC
provider signup required. (If you outgrow it later, Helius and QuickNode
both have free tiers.)

**2. Discord webhook.** Server Settings → Integrations → Webhooks → New
Webhook → copy the URL. Free, no account beyond Discord itself.

**3. Repo secrets.** In your GitHub repo: Settings → Secrets and variables
→ Actions:

| Secret | Example |
|---|---|
| `RPC_HTTP_URL` | `https://api.mainnet-beta.solana.com` |
| `WATCH_ADDRESS` | the wallet or token account to watch |
| `DISCORD_WEBHOOK_URL` | your webhook URL |

Optional repo **variables** (not secrets) to override defaults:

| Variable | Default | Meaning |
|---|---|---|
| `SOL_THRESHOLD` | `100` | minimum SOL (UI units) to trigger an alert |
| `TOKEN_THRESHOLD` | `10000` | minimum SPL token amount (UI units) to trigger an alert |

**4. Enable the workflow.** `.github/workflows/watch.yml` is already
scheduled — push it and it starts running on its own. Use the "Run
workflow" button under Actions for an on-demand test instead of waiting for
the next cron tick.

## Running locally

```bash
export RPC_HTTP_URL=https://api.mainnet-beta.solana.com
export WATCH_ADDRESS=<address to watch>
export DISCORD_WEBHOOK_URL=<your webhook>
cargo run
```

First run seeds `state/last_seen_signature.json` with the current latest
signature and exits without alerting — it won't replay a wallet's entire
history as a wall of alerts the first time it sees it. Every run after that
only looks at what's new.

## Tests

```bash
cargo test
```

`src/decode.rs` has unit tests covering SOL transfers, `transferChecked`
SPL transfers, and instructions that should be ignored — all built from
realistic `jsonParsed` response shapes, so the decode logic is verified
without needing live network access.

## Known limitations

These are deliberate scope decisions, not bugs I haven't gotten to:

- **Top-level instructions only.** A transfer routed through a CPI call —
  inside a swap, for instance — lives in `meta.innerInstructions`, not the
  top-level instruction list, and isn't decoded here. Extending
  `decode_transfers` to also walk `innerInstructions` is a natural next
  step; it's left out to keep the decode path simple to audit.
- **`transferChecked` only, not legacy `transfer`.** The older SPL
  instruction doesn't carry decimals, so converting its amount to UI units
  needs a separate mint lookup. `transferChecked` has been the norm for
  years, so the practical coverage gap is small — but it exists.
- **Single address, not a whole token.** This watches one wallet or token
  account, not "all transfers of mint X across the network." Genuinely
  network-wide monitoring needs an indexer (or a Geyser plugin), which is a
  different, much bigger project.
- **Cron cadence, not guaranteed.** GitHub's scheduler is best-effort and
  can lag under load — treat "every 10 minutes" as "at least every ~10-15
  minutes," not a hard SLA.

## License

MIT
