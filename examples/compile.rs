// usage:  cargo run --release --example compile

use fasteval3::Compiler;
use fasteval3::{EmptyNamespace, Evaler}; // use this trait so we can call eval().
use std::collections::BTreeMap; // use this trait so we can call compile().
fn main() -> Result<(), fasteval3::Error> {
    let parser = fasteval3::Parser::new();
    let mut slab = fasteval3::Slab::new();
    let mut map = BTreeMap::new();

    let expr_str = "sin(deg/360 * 2*pi())";
    let compiled = parser
        .parse(expr_str, &mut slab.ps)?
        .from(&slab.ps)
        .compile(&slab.ps, &mut slab.cs, &mut EmptyNamespace);
    for deg in 0..360 {
        map.insert(String::from("deg"), deg as f64);
        // When working with compiled constant expressions, you can use the
        // eval_compiled*!() macros to save a function call:
        let val = fasteval3::eval_compiled!(compiled, &slab, &mut map);
        eprintln!("sin({}°) = {}", deg, val);
    }

    Ok(())
}
