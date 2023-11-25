// usage:  cargo run --release --example simple-vars

use std::collections::BTreeMap;
fn main() -> Result<(), fasteval3::Error> {
    let mut map: BTreeMap<String, f64> = BTreeMap::new();
    map.insert(String::from("x"), 1.0);
    map.insert(String::from("y"), 2.0);
    map.insert(String::from("z"), 3.0);

    let val = fasteval3::ez_eval(r#"x + print("y:",y) + z"#, &mut map)?;
    //                                 |
    //                                 prints "y: 2" to stderr and then evaluates to 2.0

    assert_eq!(val, 6.0);

    Ok(())
}
