// usage:  cargo run --release --example ns_callback

fn main() -> Result<(), fasteval3::Error> {
    let mut num_lookups = 0;
    let mut cb = |name: &str, _args: Vec<f32>| -> Option<f32> {
        num_lookups += 1;
        match name {
            "x" => Some(2.0),
            _ => None,
        }
    };

    let val = fasteval3::ez_eval("x * (x + 1)", &mut cb)?;
    assert!((val - 6.0).abs() < f32::EPSILON);
    assert_eq!(num_lookups, 2); // Notice that 'x' was looked-up twice.

    Ok(())
}
