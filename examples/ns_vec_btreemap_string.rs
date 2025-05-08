// usage:  cargo run --release --example ns_vec_btreemap_string

use std::collections::BTreeMap;
fn main() -> Result<(), fasteval3::Error> {
    let mut layer1 = BTreeMap::new();
    layer1.insert(String::from("x"), 2.0);
    layer1.insert(String::from("y"), 3.0);

    let mut layered_namespace: Vec<BTreeMap<String, f32>> = vec![layer1];

    let val = fasteval3::ez_eval("x * y", &mut layered_namespace)?;
    assert!((val - 6.0).abs() < f32::EPSILON);

    // Let's add another layer which shadows the previous one:
    let mut layer2 = BTreeMap::new();
    layer2.insert(String::from("x"), 3.0);
    layered_namespace.push(layer2);

    let val = fasteval3::ez_eval("x * y", &mut layered_namespace)?;
    assert!((val - 9.0).abs() < f32::EPSILON);

    // Remove the top layer and we'll be back to what we had before:
    layered_namespace.pop();

    let val = fasteval3::ez_eval("x * y", &mut layered_namespace)?;
    assert!((val - 6.0).abs() < f32::EPSILON);

    Ok(())
}
