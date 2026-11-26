//! Stage 5 — Usage-Based Pricing (UBP) & Monetization.
//!
//! * `meter`   — API metering: idempotent event ingest, sum / max / unique-count aggregation
//!   per customer and billing period, late and duplicate events rejected with reasons.
//! * `rate`    — Stripe-style metered pricing: graduated or volume tiers, included units,
//!   platform fee and minimum commit, priced in whole cents.
//! * `credits` — prepaid credit wallets: grants burn soonest-expiring first, expiry,
//!   overage, and runway at the current burn rate.

mod api;
pub mod credits;
pub mod meter;
pub mod rate;
pub mod util;

pub use api::dispatch;
use serde_json::Value;

pub const OPS: &[&str] = &["meter_usage", "rate_invoice", "credit_ledger"];

pub fn handle(op: &str, i: &Value) -> Result<Value, String> {
    match op {
        "meter_usage" => meter::meter_op(i),
        "rate_invoice" => rate::invoice_op(i),
        "credit_ledger" => credits::ledger_op(i),
        _ => Err(format!("unknown op `{op}`; known: {}", OPS.join(", "))),
    }
}
