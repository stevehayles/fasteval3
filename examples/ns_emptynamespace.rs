// usage:  cargo run --release --example ns_emptynamespace

fn main() -> Result<(), fasteval3::Error> {
    let mut ns = fasteval3::EmptyNamespace;

    let val = fasteval3::ez_eval("sin(pi()/2)", &mut ns)?;
    assert_eq!(val, 1.0);

    Ok(())
}
