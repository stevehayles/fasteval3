// usage:  cargo run --release --example ns_btreemap_str

use std::collections::BTreeMap;
fn main() -> Result<(), fasteval3::Error> {
    let mut map: BTreeMap<&'static str, f64> = BTreeMap::new();
    map.insert("x", 2.0);

    let val = fasteval3::ez_eval("x * (x + 1)", &mut map)?;
    assert!((val - 6.0).abs() < f64::EPSILON);

    Ok(())
}
