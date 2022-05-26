// usage:  cargo run --release --example ns_emptynamespace

fn main() -> Result<(), fasteval2::Error> {
    let mut ns = fasteval2::EmptyNamespace;

    let val = fasteval2::ez_eval("sin(pi()/2)", &mut ns)?;
    assert_eq!(val, 1.0);

    Ok(())
}
