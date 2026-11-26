//! Rating. Tiers: `up_to` (null = no limit), `unit_cents` (may be fractional), `flat_cents`.
//! Graduated: each unit pays its own tier's price. Volume: all units pay the price of the tier
//! the total falls in. Each line is rounded to whole cents once. Minimum commit tops up.
use crate::util::*;
use serde_json::{json, Value};

#[derive(Clone, Copy)]
pub struct Tier {
    pub up_to: Option<f64>,
    pub unit: f64,
    pub flat: f64,
}

pub fn tiers(v: &Value) -> R<Vec<Tier>> {
    let a = v.as_array().ok_or("`tiers` must be an array")?;
    let mut out = vec![];
    let mut last = 0.0;
    for (n, t) in a.iter().enumerate() {
        let up = t.get("up_to").and_then(Value::as_f64);
        if let Some(u) = up {
            if u <= last {
                return Err(format!("tier {n}: up_to must increase"));
            }
            last = u;
        } else if n + 1 != a.len() {
            return Err("only the last tier may have up_to = null".into());
        }
        out.push(Tier { up_to: up, unit: f_or(t, "unit_cents", 0.0), flat: f_or(t, "flat_cents", 0.0) });
    }
    if out.last().map_or(true, |t| t.up_to.is_some()) {
        return Err("last tier must have up_to = null".into());
    }
    Ok(out)
}

/// Price in fractional cents (rounded by the caller).
pub fn price(q: f64, ts: &[Tier], model: &str) -> R<(f64, Vec<Value>)> {
    let mut detail = vec![];
    match model {
        "graduated" => {
            let mut lo = 0.0;
            let mut total = 0.0;
            for t in ts {
                if q <= lo {
                    break;
                }
                let hi = t.up_to.unwrap_or(f64::INFINITY);
                let units = q.min(hi) - lo;
                let c = units * t.unit + t.flat;
                detail.push(json!({"from": lo, "to": t.up_to, "units": rd(units, 4), "unit_cents": t.unit, "cents": rd(c, 4)}));
                total += c;
                lo = hi;
            }
            Ok((total, detail))
        }
        "volume" => {
            let t = ts.iter().find(|t| t.up_to.map_or(true, |u| q <= u)).unwrap();
            let c = q * t.unit + if q > 0.0 { t.flat } else { 0.0 };
            detail.push(json!({"tier_up_to": t.up_to, "units": q, "unit_cents": t.unit, "cents": rd(c, 4)}));
            Ok((c, detail))
        }
        _ => Err(format!("unknown pricing model `{model}`")),
    }
}

pub fn invoice_op(i: &Value) -> R<Value> {
    let plan = i.get("plan").ok_or("`plan` is required")?;
    let usage = obj(i, "usage")?;
    let mut lines = vec![];
    let mut sub: i64 = 0;
    let base = f_or(plan, "platform_fee_cents", 0.0).round() as i64;
    if base > 0 {
        lines.push(json!({"item": "platform fee", "cents": base}));
        sub += base;
    }
    for m in arr(plan, "meters")? {
        let name = s(m, "meter")?;
        let used = usage.get(name).and_then(Value::as_f64).unwrap_or(0.0);
        let incl = f_or(m, "included_units", 0.0);
        let billable = (used - incl).max(0.0);
        let (c, detail) = price(billable, &tiers(m.get("tiers").unwrap_or(&Value::Null))?, s_or(m, "model", "graduated"))?;
        let cents = c.round() as i64;
        sub = sub.checked_add(cents).ok_or("invoice overflow")?;
        lines.push(json!({"item": name, "used": used, "included": incl, "billable": billable, "cents": cents, "tiers": detail}));
    }
    let commit = f_or(plan, "minimum_commit_cents", 0.0).round() as i64;
    let topup = (commit - sub).max(0);
    if topup > 0 {
        lines.push(json!({"item": "minimum commit true-up", "cents": topup}));
    }
    let total = sub + topup;
    Ok(json!({"lines": lines, "total_cents": total, "total": format!("${}.{:02}", total / 100, total % 100)}))
}

#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn graduated_vs_volume() {
        let ts = tiers(&json!([{"up_to":100,"unit_cents":10},{"up_to":null,"unit_cents":5}])).unwrap();
        assert_eq!(price(150.0, &ts, "graduated").unwrap().0, 1250.0);
        assert_eq!(price(150.0, &ts, "volume").unwrap().0, 750.0);
        assert!(tiers(&json!([{"up_to":null},{"up_to":5}])).is_err());
    }
    #[test]
    fn included_and_commit() {
        let i = json!({"usage":{"calls":120},"plan":{"platform_fee_cents":1000,"minimum_commit_cents":5000,
            "meters":[{"meter":"calls","included_units":100,"tiers":[{"up_to":null,"unit_cents":10}]}]}});
        let r = invoice_op(&i).unwrap();
        assert_eq!(r["total_cents"], 5000);
        assert_eq!(r["lines"][1]["cents"], 200);
    }
}
