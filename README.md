# gtm-05-usage-billing

**GTM Engineering — Stage 5 of 12: Usage-Based Pricing (UBP) & Monetization**

Bill for value, not seats: idempotent API metering, Stripe-style graduated and volume tiers with included units and minimum commits, and prepaid credit wallets with expiry and runway alerts.

**Live demo:** https://gtm-05-usage-billing.braylo777.workers.dev — the Rust core runs in your browser as WebAssembly. No server, no tracking.

## Why this matters

Usage-based pricing ties revenue to the value a customer gets, but it moves the hard part into engineering. A retried webhook billed twice, a late event moved into the wrong month, or a float rounding error on 40 million API calls is a billing dispute. Credit systems add expiry rules finance must sign off on. This repo makes metering, rating and credits deterministic and testable.

## What it does

- Metering: idempotency keys, sum / max / unique-count / count aggregation, period bounds, duplicate / late / unknown-meter rejections with counts
- Rating: graduated and volume tiers, fractional unit prices, flat fees per tier, included units, platform fee, minimum-commit true-up, one rounding per line in whole cents
- Tier validation: increasing up_to, only the last tier unbounded
- Credit wallets: soonest-expiring grant burns first, daily expiry, overage, balance, burn rate, runway days, low-balance alert

## What it does not do (yet)

- No Stripe API calls: the output mirrors metered-billing invoice lines but is not pushed to Stripe
- No proration for mid-period plan changes, no tax, single currency
- Outcome-based pricing (pay per resolved ticket, per meeting booked) needs an outcome event feed; the meter accepts it but no outcome model is included

## Operations

Every operation takes JSON in and gives JSON out. The CLI, the tests and the demo page all use the same entry point.

| Operation | What it does |
|---|---|
| `meter_usage` | Aggregate a 30-day event stream per customer. The sample has one retried duplicate, one late event and one typo meter. |
| `rate_invoice` | Price usage with a platform fee, included units, graduated API tiers, volume storage tiers and a minimum commit. |
| `credit_ledger` | Burn daily usage from prepaid grants, soonest expiry first. Expired promo credits and runway are reported. |

## Run it

You need Rust (stable). No Node, no npm.

```bash
git clone https://github.com/braylo777/gtm-05-usage-billing
cd gtm-05-usage-billing
cargo test
cargo run -- ops
```

Try the first demo example from the command line:

```bash
python3 -c 'import json;print(json.dumps(json.load(open("web/examples.json"))["ops"][0]["input"]))' | cargo run -q -- meter_usage
```

## Build and deploy the demo

```bash
./build.sh    # tests, then compiles the core to web/app.wasm
./deploy.sh   # serves web/ as a Cloudflare Workers static-assets site (needs a Cloudflare login)
```

`web/` is a plain static site (`index.html`, `app.js`, `app.wasm`). It has no build step, so any static host works.

## Layout

```
src/lib.rs        operation list and router
src/api.rs        JSON entry point + the 3 WASM exports (alloc, dealloc, run)
src/main.rs       CLI
src/*.rs          one module per capability, each with unit tests
tests/            integration tests (every demo example must pass)
web/              demo page; examples.json drives both the page and this README
```

## Design choices

- **Rust, few dependencies.** Only `serde_json`, pinned to exact versions with a committed `Cargo.lock`.
- **Deterministic.** Time, randomness and keys are inputs, so every result can be replayed in a test or an audit.
- **Fail closed.** Unknown input or missing fields give an error, never a silent default.

## Part of a series

This is one of 12 repos that work through a public Go-To-Market (GTM) Engineering roadmap: [12 stages by Suraj Sharma (@suraj_sharma14)](https://x.com/suraj_sharma14/status/2125552583452270692). The roadmap is the inspiration; the code here is original. All 12: [github.com/braylo777?q=gtm-](https://github.com/braylo777?tab=repositories&q=gtm-).

## Data and privacy

Every example uses synthetic data. Nothing calls a paid API, scrapes a site, identifies a real visitor or sends an email.

## License

MIT — see [LICENSE](LICENSE). Built by [Braylo SI](https://braylo.si).
