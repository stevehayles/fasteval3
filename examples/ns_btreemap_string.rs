// usage:  cargo run --release --example ns_btreemap_string

use std::collections::BTreeMap;
fn main() -> Result<(), fasteval3::Error> {
    let mut map: BTreeMap<String, f64> = BTreeMap::new();
    map.insert(String::from("x"), 2.0);

    let val = fasteval3::ez_eval("x * (x + 1)", &mut map)?;
    assert!((val - 6.0).abs() < f64::EPSILON);

    Ok(())
}
