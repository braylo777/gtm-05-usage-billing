//! Metering. Each event has an idempotency key; a retry with the same key counts once.
//! Events outside [period_start, period_end) are rejected, not silently moved.
use crate::util::*;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

pub fn meter_op(i: &Value) -> R<Value> {
    let start = f(i, "period_start_day")?;
    let end = f(i, "period_end_day")?;
    if end <= start {
        return Err("period_end_day must be after period_start_day".into());
    }
    let defs = obj(i, "meters")?;
    let mut seen = BTreeSet::new();
    let mut agg: BTreeMap<(String, String), (f64, f64, BTreeSet<String>, u64)> = BTreeMap::new();
    let (mut dup, mut late, mut unknown) = (0, 0, 0);
    for e in arr(i, "events")? {
        let key = s(e, "idempotency_key")?;
        if !seen.insert(key.to_string()) {
            dup += 1;
            continue;
        }
        let day = f(e, "day")?;
        if day < start || day >= end {
            late += 1;
            continue;
        }
        let m = s(e, "meter")?;
        if !defs.contains_key(m) {
            unknown += 1;
            continue;
        }
        let v = f_or(e, "value", 1.0);
        if v < 0.0 {
            return Err(format!("event {key}: negative usage is not allowed (use a credit note)"));
        }
        let x = agg.entry((s(e, "customer")?.to_string(), m.to_string())).or_insert((0.0, 0.0, BTreeSet::new(), 0));
        x.0 += v;
        x.1 = x.1.max(v);
        if let Some(u) = e.get("unique_id").and_then(Value::as_str) {
            x.2.insert(u.to_string());
        }
        x.3 += 1;
    }
    let mut out: BTreeMap<String, Map<String, Value>> = BTreeMap::new();
    for ((c, m), (sum, max, uniq, n)) in agg {
        let how = defs[&m].get("aggregation").and_then(Value::as_str).unwrap_or("sum");
        let q = match how {
            "sum" => sum,
            "max" => max,
            "unique_count" => uniq.len() as f64,
            "count" => n as f64,
            _ => return Err(format!("meter {m}: unknown aggregation `{how}`")),
        };
        out.entry(c).or_default().insert(m, json!(rd(q, 4)));
    }
    Ok(json!({"usage": out, "rejected": {"duplicate": dup, "outside_period": late, "unknown_meter": unknown}}))
}

#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn idempotent_and_aggregations() {
        let i = json!({"period_start_day":0,"period_end_day":30,"meters":{"calls":{"aggregation":"sum"},"mau":{"aggregation":"unique_count"},"gb":{"aggregation":"max"}},
            "events":[{"idempotency_key":"a","customer":"c","meter":"calls","value":5,"day":1},{"idempotency_key":"a","customer":"c","meter":"calls","value":5,"day":1},
            {"idempotency_key":"b","customer":"c","meter":"mau","unique_id":"u1","day":2},{"idempotency_key":"c","customer":"c","meter":"mau","unique_id":"u1","day":3},
            {"idempotency_key":"d","customer":"c","meter":"gb","value":7,"day":3},{"idempotency_key":"e","customer":"c","meter":"gb","value":4,"day":4},
            {"idempotency_key":"f","customer":"c","meter":"calls","value":1,"day":30}]});
        let r = meter_op(&i).unwrap();
        assert_eq!(r["usage"]["c"], json!({"calls":5.0,"gb":7.0,"mau":1.0}));
        assert_eq!(r["rejected"], json!({"duplicate":1,"outside_period":1,"unknown_meter":0}));
    }
}
