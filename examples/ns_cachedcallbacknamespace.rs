// usage:  cargo run --release --example ns_cachedcallbacknamespace

fn main() -> Result<(), fasteval3::Error> {
    let mut num_lookups = 0;
    let val = {
        let cb = |name: &str, _args: Vec<f64>| -> Option<f64> {
            num_lookups += 1;
            match name {
                "x" => {
                    // Pretend that it is very expensive to calculate this,
                    // and that's why we want to use the CachedCallbackNamespace cache.
                    for _ in 0..1_000_000 { /* do work */ } // Fake Work for this example.
                    Some(2.0)
                }
                _ => None,
            }
        };
        let mut ns = fasteval3::CachedCallbackNamespace::new(cb);

        fasteval3::ez_eval("x * (x + 1)", &mut ns)?
    };
    assert!((val - 6.0).abs() < f64::EPSILON);
    assert_eq!(num_lookups, 1); // Notice that only 1 lookup occurred.
                                // The second 'x' value was cached.

    Ok(())
}
