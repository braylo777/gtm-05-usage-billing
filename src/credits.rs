//! Credit ledger. Each day: expire grants whose `expires_day` has passed, then burn usage from
//! the grant that expires soonest. Usage with no credit left becomes overage.
use crate::util::*;
use serde_json::{json, Value};

struct Grant {
    id: String,
    left: f64,
    granted: f64,
    expires: f64,
    expired: f64,
}

pub fn ledger_op(i: &Value) -> R<Value> {
    let mut gs = vec![];
    for g in arr(i, "grants")? {
        let amt = f(g, "credits")?;
        if amt < 0.0 {
            return Err("grant credits must be ≥ 0".into());
        }
        gs.push(Grant { id: s(g, "id")?.into(), left: amt, granted: f(g, "granted_day")?, expires: f_or(g, "expires_day", f64::INFINITY), expired: 0.0 });
    }
    let mut usage: Vec<(f64, f64)> = vec![];
    for u in arr(i, "daily_usage")? {
        usage.push((f(u, "day")?, f(u, "credits")?));
    }
    usage.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut overage = 0.0;
    let mut log = vec![];
    for (day, mut need) in usage.iter().copied() {
        for g in gs.iter_mut() {
            if g.expires <= day && g.left > 0.0 {
                g.expired += g.left;
                log.push(json!({"day": day, "event": "expired", "grant": g.id, "credits": rd(g.left, 2)}));
                g.left = 0.0;
            }
        }
        let mut live: Vec<&mut Grant> = gs.iter_mut().filter(|g| g.granted <= day && g.left > 0.0).collect();
        live.sort_by(|a, b| a.expires.total_cmp(&b.expires).then(a.granted.total_cmp(&b.granted)));
        for g in live {
            if need <= 0.0 {
                break;
            }
            let take = need.min(g.left);
            g.left -= take;
            need -= take;
        }
        if need > 0.0 {
            overage += need;
            log.push(json!({"day": day, "event": "overage", "credits": rd(need, 2)}));
        }
    }
    let as_of = f_or(i, "as_of_day", usage.last().map_or(0.0, |u| u.0));
    let balance: f64 = gs.iter().filter(|g| g.expires > as_of && g.granted <= as_of).map(|g| g.left).sum();
    let window = f_or(i, "burn_window_days", 14.0);
    let recent: f64 = usage.iter().filter(|u| u.0 > as_of - window && u.0 <= as_of).map(|u| u.1).sum();
    let burn = recent / window;
    let runway = if burn > 0.0 { Some(rd(balance / burn, 1)) } else { None };
    let grants: Vec<Value> = gs.iter().map(|g| json!({"id": g.id, "left": rd(g.left, 2), "expired": rd(g.expired, 2), "expires_day": if g.expires.is_finite() { json!(g.expires) } else { Value::Null }})).collect();
    let low = f_or(i, "low_balance_alert_days", 7.0);
    Ok(json!({"balance": rd(balance, 2), "burn_per_day": rd(burn, 2), "runway_days": runway, "overage_credits": rd(overage, 2),
        "alert": runway.map_or(false, |r| r < low), "grants": grants, "events": log}))
}

#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn soonest_expiry_first_then_expire_then_overage() {
        let i = json!({"grants":[{"id":"promo","credits":10,"granted_day":0,"expires_day":5},{"id":"paid","credits":20,"granted_day":0}],
            "daily_usage":[{"day":1,"credits":4},{"day":6,"credits":25}],"as_of_day":6,"burn_window_days":7});
        let r = ledger_op(&i).unwrap();
        assert_eq!(r["grants"][0]["expired"], 6.0);
        assert_eq!(r["overage_credits"], 5.0);
        assert_eq!(r["balance"], 0.0);
    }
}
