// usage:  cargo run --release --example compile

use fasteval3::Compiler;
use fasteval3::Evaler; // use this trait so we can call eval().
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::Instant;

fn main() -> Result<(), fasteval3::Error> {
    let parser = fasteval3::Parser::new();
    let mut slab = fasteval3::Slab::new();
    let map = Rc::new(RefCell::new(BTreeMap::<&str, f32>::new()));

    let mut cb = |name: &str, args: Vec<f32>| -> Option<f32> {
        let mydata: [f32; 3] = [11.1, 22.2, 33.3];

        match name {
            n if map.borrow().contains_key(n) => Some(*map.borrow().get(n).unwrap()),

            // Custom constants/variables:
            "X" | "x" => Some(3.0),
            "Y" | "y" => Some(4.0),

            // Custom function:
            "sum" => Some(args.into_iter().fold(0.0, |s, f| s + f)),

            // Custom array-like objects:
            // The `args.get...` code is the same as:
            //     mydata[args[0] as usize]
            // ...but it won't panic if either index is out-of-bounds.
            "data" => args.first().and_then(|f| mydata.get(*f as usize).copied()),

            // A wildcard to handle all undefined names:
            _ => None,
        }
    };

    //let expr_str = "sin(deg/360 * 2*pi())";
    //let expr_str = "(1/(a+1)+2/(a+2)+3/(a+3))";
    //let expr_str = "a * a * ((((87))) - 73) + (97 + (((15 / 55 * ((31)) + 35))) + (15 - (9)) - (39 / 26) / 20 / 91 + 27 / (33 * 26 + 28 - (7) / 10 + 66 * 6) + 60 / 35 - ((29) - (69) / 44 / (92)) / (89) + 2 + 87 / 47 * ((2)) * 83 / 98 * 42 / (((67)) * ((97))) / (34 / 89 + 77) - 29 + 70 * (20)) + ((((((92))) + 23 * (98) / (95) + (((99) * (41))) + (5 + 41) + 10) - (36) / (6 + 80 * 52 + (90))))";
    let expr_str =
        "(sum(1+2+3+a) + x + 1+2*3/4^5%6 + log(100K) + log(e(),100) + [3*(3-3)/3] + (2<3) + data[a % 3]) * cos(60 * (pi() / 180))";

    let compiled = parser
        .parse(expr_str, &mut slab.ps)?
        .from(&slab.ps)
        .compile(&slab.ps, &mut slab.cs, &mut cb);

    let start = Instant::now();

    const N: i32 = 100_000;

    for deg in 0..=N {
        map.borrow_mut().insert("a", f64::from(deg) as f32);
        // When working with compiled constant expressions, you can use the
        // eval_compiled*!() macros to save a function call:
        let val = fasteval3::eval_compiled!(compiled, &slab, &mut cb);

        if deg == N || deg % (N / 5) == 0 {
            eprintln!("val = {val}");
        }
    }

    let eval_time = start.elapsed().as_millis();
    eprintln!("elapsed millis = {eval_time}");

    Ok(())
}
