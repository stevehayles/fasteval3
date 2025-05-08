pub(crate) mod common;

use common::assert_error_margin;

use fasteval3::ez_eval;

#[test]
fn empty() {
    let mut ns = fasteval3::EmptyNamespace;

    let val = ez_eval("1 + 1", &mut ns).unwrap();
    assert_error_margin(val, 2.0);
}

#[test]
fn str_to_f32() {
    {
        let mut ns = fasteval3::StringTof32Namespace::new();
        ns.insert(String::from("a"), 1.11);
        ns.insert(String::from("b"), 2.22);

        let val = ez_eval("a + b + 1", &mut ns).unwrap();
        assert_error_margin(val, 4.33);
    }

    {
        let mut ns = fasteval3::StrTof32Namespace::new();
        ns.insert("a", 1.11);
        ns.insert("b", 2.22);

        let val = ez_eval("a + b + 1", &mut ns).unwrap();
        assert_error_margin(val, 4.33);
    }
}

#[test]
fn str_to_cb() {
    {
        let mut ns = fasteval3::StringToCallbackNamespace::new();
        ns.insert(String::from("a"), Box::new(|args| args[0]));
        ns.insert(String::from("b"), Box::new(|args| args[0] * 2.0));

        let val = ez_eval("a(1.11) + b(1.11) + 1", &mut ns).unwrap();
        assert_error_margin(val, 4.33);
    }

    {
        let mut ns = fasteval3::StrToCallbackNamespace::new();
        ns.insert("a", Box::new(|args| args[0]));
        ns.insert("b", Box::new(|args| args[0] * 2.0));

        let val = ez_eval("a(1.11) + b(1.11) + 1", &mut ns).unwrap();
        assert_error_margin(val, 4.33);
    }
}

#[test]
fn layered_str_to_f32() {
    let mut ns = fasteval3::LayeredStringTof32Namespace::new();
    let mut layer0 = fasteval3::StringTof32Namespace::new();
    layer0.insert(String::from("a"), 1.11);
    layer0.insert(String::from("b"), 2.22);
    ns.push(layer0);

    let val = ez_eval("a + b + 1", &mut ns).unwrap();
    assert_error_margin(val, 4.33);

    let mut layer1 = fasteval3::StringTof32Namespace::new();
    layer1.insert(String::from("a"), 11.11);
    ns.push(layer1);

    let val = ez_eval("a + b + 1", &mut ns).unwrap();
    assert_error_margin(val, 14.33);

    ns.pop();

    let val = ez_eval("a + b + 1", &mut ns).unwrap();
    assert_error_margin(val, 4.33);
}

#[test]
fn cb() {
    let mut ns = |name: &str, args: Vec<f32>| match name {
        "a" => Some(1.11),
        "b" => Some(2.22),
        "len" => Some(args.len() as f32),
        _ => None,
    };

    let val = ez_eval("a + b + 1", &mut ns).unwrap();
    assert_error_margin(val, 4.33);
}

#[test]
fn cached_cb() {
    let mut ns = fasteval3::CachedCallbackNamespace::new(|name: &str, args: Vec<f32>| match name {
        "a" => {
            eprintln!("cached_cb: a: This should only be printed once.");
            Some(1.11)
        }
        "b" => Some(2.22),
        "len" => Some(args.len() as f32),
        _ => None,
    });

    let val = ez_eval("a + b + 1", &mut ns).unwrap();
    assert_error_margin(val, 4.33);
    ez_eval("a + b + 1", &mut ns).unwrap();
    ez_eval("a + b + 1", &mut ns).unwrap();
    ez_eval("a + b + 1", &mut ns).unwrap();
    ez_eval("a + b + 1", &mut ns).unwrap();
}

#[test]
fn custom_vector_funcs() {
    let vecs_cell = std::cell::RefCell::new(Vec::<Vec<f32>>::new());

    let mut ns = fasteval3::StrToCallbackNamespace::new();

    ns.insert("x", Box::new(|_args| 2.0));

    ns.insert(
        "vec_store",
        Box::new(|args| {
            let mut vecs = vecs_cell.borrow_mut();
            let index = vecs.len();
            vecs.push(args);
            index as f32
        }),
    );

    ns.insert(
        "vec_sum",
        Box::new(|args| {
            if let Some(index) = args.first() {
                if let Some(v) = vecs_cell.borrow().get(*index as usize) {
                    return v.iter().sum();
                }
            }
            f32::NAN
        }),
    );

    let val = ez_eval("vec_sum(vec_store(1.1, x, 3.3)) + vec_sum(0)", &mut ns).unwrap();
    assert_error_margin(val, 12.799999);
}
