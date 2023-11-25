use fasteval3::{ez_eval, Error};

use std::collections::BTreeMap;

#[test]
fn ez() {
    assert_eq!(
        ez_eval("3+3-3/3", &mut BTreeMap::<String, f64>::new()),
        Ok(5.0)
    );
    assert_eq!(
        ez_eval("3abc+3-3/3", &mut BTreeMap::<String, f64>::new()),
        Err(Error::UnparsedTokensRemaining(String::from("abc+3-3/3")))
    );
    assert_eq!(
        ez_eval("z+z-z/z", &mut {
            let mut m = BTreeMap::<String, f64>::new();
            m.insert(String::from("x"), 1.0);
            m.insert(String::from("y"), 2.0);
            m.insert(String::from("z"), 3.0);
            m
        }),
        Ok(5.0)
    );
}
