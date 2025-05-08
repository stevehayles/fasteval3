// usage:  cargo run --release --example compile

use fasteval3::Compiler;
use fasteval3::{EmptyNamespace, Evaler}; // use this trait so we can call eval().
use std::collections::BTreeMap; // use this trait so we can call compile().
use std::time::Instant;

fn main() -> Result<(), fasteval3::Error> {
    let parser = fasteval3::Parser::new();
    let mut slab = fasteval3::Slab::new();
    let mut map = BTreeMap::new();

    //let expr_str = "sin(deg/360 * 2*pi())";
    //let expr_str = "(1/(a+1)+2/(a+2)+3/(a+3))";
    let expr_str = "a * ((((87))) - 73) + (97 + (((15 / 55 * ((31)) + 35))) + (15 - (9)) - (39 / 26) / 20 / 91 + 27 / (33 * 26 + 28 - (7) / 10 + 66 * 6) + 60 / 35 - ((29) - (69) / 44 / (92)) / (89) + 2 + 87 / 47 * ((2)) * 83 / 98 * 42 / (((67)) * ((97))) / (34 / 89 + 77) - 29 + 70 * (20)) + ((((((92))) + 23 * (98) / (95) + (((99) * (41))) + (5 + 41) + 10) - (36) / (6 + 80 * 52 + (90))))";
    let compiled = parser
        .parse(expr_str, &mut slab.ps)?
        .from(&slab.ps)
        .compile(&slab.ps, &mut slab.cs, &mut EmptyNamespace);

    let start = Instant::now();

    for deg in 0..100_000_i32 {
        map.insert(String::from("a"), f64::from(deg) as f32);
        // When working with compiled constant expressions, you can use the
        // eval_compiled*!() macros to save a function call:
        let val = fasteval3::eval_compiled!(compiled, &slab, &mut map);
        //eprintln!("val = {val}");
    }

    let eval_time = start.elapsed().as_millis();
    eprintln!("elapsed millis = {eval_time}");

    Ok(())
}
