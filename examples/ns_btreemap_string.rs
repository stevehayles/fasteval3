// usage:  cargo run --release --example ns_btreemap_string

use std::collections::BTreeMap;
fn main() -> Result<(), fasteval2::Error> {
    let mut map: BTreeMap<String, f64> = BTreeMap::new();
    map.insert("x".to_string(), 2.0);

    let val = fasteval2::ez_eval("x * (x + 1)", &mut map)?;
    assert_eq!(val, 6.0);

    Ok(())
}
