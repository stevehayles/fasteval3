//! Evaluation Namespaces used for Variable-lookups and custom Functions.
//!
//! Several Evaluation Namespace types are defined, each with their own advantages:
//! * [`EmptyNamespace`](#emptynamespace) -- Useful when you know that your
//!   expressions don't need to look up any variables.
//! * `BTreeMap` -- A simple way to define variables and functions with a map.
//!   Type aliases: [`StringTof32Namespace`](#stringtof32namespace),
//!   [`StrTof32Namespace`](#strtof32namespace),
//!   [`StringToCallbackNamespace`](#stringtocallbacknamespace),
//!   [`StrToCallbackNamespace`](#strtocallbacknamespace)
//! * [`FnMut(&str,Vec<f32>) -> Option<f32>`](#callback-fnmutstrvec---option) --
//!   Define variables and custom functions using a callback function.
//! * [`CachedCallbackNamespace`](#cachedcallbacknamespace) -- Like the above
//!   callback-based Namespace, but results are cached so the callback is not
//!   queried more than once for a given variable.
//! * `Vec<BTreeMap<String,f32>>` -- Define variables with layered maps.
//!   Each layer is a separate 'scope'.  Higher layers take precedence
//!   over lower layers.  Very useful for creating scoped higher-level-languages.
//!   Type alias: [`LayeredStringTof32Namespace`](#layeredstringtof32namespace)
//!
//! # Examples
//!
//! ## `EmptyNamespace`
//! ```
//! fn main() -> Result<(), fasteval3::Error> {
//!     let mut ns = fasteval3::EmptyNamespace;
//!
//!     let val = fasteval3::ez_eval("sin(pi()/2)", &mut ns)?;
//!     assert_eq!(val, 1.0);
//!
//!     Ok(())
//! }
//! ```
//!
//! ## `StringTof32Namespace`
//! ```
//! fn main() -> Result<(), fasteval3::Error> {
//!     let mut ns = fasteval3::StringTof32Namespace::new();
//!     ns.insert("x".to_string(), 2.0);
//!
//!     let val = fasteval3::ez_eval("x * (x + 1)", &mut ns)?;
//!     assert_eq!(val, 6.0);
//!
//!     Ok(())
//! }
//! ```
//!
//! ## `StrTof32Namespace`
//! ```
//! fn main() -> Result<(), fasteval3::Error> {
//!     let mut ns = fasteval3::StrTof32Namespace::new();
//!     ns.insert("x", 2.0);
//!
//!     let val = fasteval3::ez_eval("x * (x + 1)", &mut ns)?;
//!     assert_eq!(val, 6.0);
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Callback: FnMut(&str,Vec<f32>) -> Option<f32>
//! ```
//! fn main() -> Result<(), fasteval3::Error> {
//!     let mut num_lookups = 0;
//!     let mut cb = |name:&str, args:Vec<f32>| -> Option<f32> {
//!         num_lookups += 1;
//!         match name {
//!             "x" => Some(2.0),
//!             _ => None,
//!         }
//!     };
//!
//!     let val = fasteval3::ez_eval("x * (x + 1)", &mut cb)?;
//!     assert_eq!(val, 6.0);
//!     assert_eq!(num_lookups, 2);  // Notice that 'x' was looked-up twice.
//!
//!     Ok(())
//! }
//! ```
//!
//! ## `StringToCallbackNamespace`
//! ```
//! fn main() -> Result<(), fasteval3::Error> {
//!     let mut ns = fasteval3::StringToCallbackNamespace::new();
//!     ns.insert("x".to_string(), Box::new(|_args| 2.0));
//!     ns.insert("double".to_string(), Box::new(|args| {
//!         args.get(0).map(|arg0| arg0*2.0).unwrap_or(std::f32::NAN)
//!     }));
//!
//!     let val = fasteval3::ez_eval("double(x + 1) + 1", &mut ns)?;
//!     assert_eq!(val, 7.0);
//!
//!     Ok(())
//! }
//! ```
//!
//! ## `StrToCallbackNamespace`
//! ```
//! fn main() -> Result<(), fasteval3::Error> {
//!     let mut ns = fasteval3::StrToCallbackNamespace::new();
//!     ns.insert("x", Box::new(|_args| 2.0));
//!     ns.insert("double", Box::new(|args| {
//!         args.get(0).map(|arg0| arg0*2.0).unwrap_or(std::f32::NAN)
//!     }));
//!
//!     let val = fasteval3::ez_eval("double(x + 1) + 1", &mut ns)?;
//!     assert_eq!(val, 7.0);
//!
//!     Ok(())
//! }
//! ```
//!
//! ## `CachedCallbackNamespace`
//! ```
//! fn main() -> Result<(), fasteval3::Error> {
//!     let mut num_lookups = 0;
//!     let val = {
//!         let cb = |name:&str, args:Vec<f32>| -> Option<f32> {
//!             num_lookups += 1;
//!             match name {
//!                 "x" => {
//!                     // Pretend that it is very expensive to calculate this,
//!                     // and that's why we want to use the CachedCallbackNamespace cache.
//!                     for i in 0..1000000 { /* do work */ }  // Fake Work for this example.
//!                     Some(2.0)
//!                 }
//!                 _ => None,
//!             }
//!         };
//!         let mut ns = fasteval3::CachedCallbackNamespace::new(cb);
//!
//!         fasteval3::ez_eval("x * (x + 1)", &mut ns)?
//!     };
//!     assert_eq!(val, 6.0);
//!     assert_eq!(num_lookups, 1);  // Notice that only 1 lookup occurred.
//!                                  // The second 'x' value was cached.
//!
//!     Ok(())
//! }
//! ```
//!
//! ## `LayeredStringTof32Namespace`
//! ```
//! fn main() -> Result<(), fasteval3::Error> {
//!     let mut layer1 = fasteval3::StringTof32Namespace::new();
//!     layer1.insert("x".to_string(), 2.0);
//!     layer1.insert("y".to_string(), 3.0);
//!
//!     let mut layers : fasteval3::LayeredStringTof32Namespace = vec![layer1];
//!
//!     let val = fasteval3::ez_eval("x * y", &mut layers)?;
//!     assert_eq!(val, 6.0);
//!
//!     // Let's add another layer which shadows the previous one:
//!     let mut layer2 = fasteval3::StringTof32Namespace::new();
//!     layer2.insert("x".to_string(), 3.0);
//!     layers.push(layer2);
//!
//!     let val = fasteval3::ez_eval("x * y", &mut layers)?;
//!     assert_eq!(val, 9.0);
//!
//!     // Remove the top layer and we'll be back to what we had before:
//!     layers.pop();
//!
//!     let val = fasteval3::ez_eval("x * y", &mut layers)?;
//!     assert_eq!(val, 6.0);
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Custom Namespace Types
//!
//! If the pre-defined Namespace types aren't perfect for your application, you
//! can create your own namespace type -- just implemenet the `EvalNamespace`
//! trait (and maybe the `Cached` and `Layered` traits too).  Also, as
//! `fasteval3` becomes more mature and is used for more real-life things, I
//! will continue to add more useful Namespace types.
//!
//! Here are a few ideas of possibly-useful custom Namespace types:
//!
//! * Vec<Fn(&str,Vec<f32>)->Option<f32>>  --  This would be a `Layered`
//!   namespace, with each layer having its own callback.  Really powerful!
//!
//! * `CachedCallbacksNamespace`  --  Same as above, but with a cache for each
//!   layer.  Good for expensive look-ups.

use crate::error::Error;

use std::collections::BTreeMap;

//---- Types:

/// All `fasteval` Namespaces must implement the `EvalNamespace` trait.
pub trait EvalNamespace {
    /// Perform a variable/function lookup.
    ///
    /// May return cached values.
    fn lookup(&mut self, name: &str, args: Vec<f32>, keybuf: &mut String) -> Option<f32>;
}

/// Cache operations for `EvalNamespace`s.
///
/// Implement this trait if your Namespace type uses a cache.
pub trait Cached {
    /// Creates a new cached entry. 
    /// 
    /// # Errors
    /// 
    /// If an entry with the same name already
    /// exists, an [`AlreadyExists` Error](../error/enum.Error.html#variant.AlreadyExists) is returned.
    fn cache_create(&mut self, name: String, val: f32) -> Result<(), Error>;

    /// Sets a cached entry.  It doesn't matter whether or not a previous value
    /// existed with this name.
    fn cache_set(&mut self, name: String, val: f32);

    /// Clear all cached entries.  Values will be recalculated and cached
    /// again the next time they are looked up.
    fn cache_clear(&mut self);
}

// I don't want to put this into the public API until it is needed.
// pub trait Layered {
//     fn push(&mut self);
//     fn pop(&mut self);
// }

/// Use `EmptyNamespace` when you know that you won't be looking up any variables.
///
/// It is a zero-sized type, which means it gets optimized-away at compile time.
///
/// [See module-level documentation for example.](index.html#emptynamespace)
///
pub struct EmptyNamespace;

/// `CachedCallbackNamespace` is useful when your variable/function lookups are expensive.
///
/// Each variable+args combo will only be looked up once, and then it will be
/// cached and re-used for subsequent lookups.
///
/// [See module-level documentation for example.](index.html#cachedcallbacknamespace)
///
pub struct CachedCallbackNamespace<'a> {
    cache: BTreeMap<String, f32>,
    cb: Box<dyn FnMut(&str, Vec<f32>) -> Option<f32> + 'a>, // I think a reference would be more efficient than a Box, but then I would need to use a funky 'let cb=|n|{}; Namespace::new(&cb)' syntax.  The Box results in a super convenient pass-the-cb-by-value API interface.
}

// I am commenting these out until I need them in real-life.
// (I don't want to add things to the public API until necessary.)
// pub struct CachedLayeredNamespace<'a> {
//     caches:Vec<BTreeMap<String,f32>>,
//     cb    :Box<dyn FnMut(&str, Vec<f32>)->Option<f32> + 'a>,
// }
// pub struct Bubble<'a,NS> where NS:EvalNamespace+Layered+'a {
//     ns   :&'a mut NS,
//     count:usize,
// }

//---- Impls:

#[allow(clippy::inline_always)] // TODO: Check to see if `always inlining here is ok.
#[inline(always)]
fn key_from_nameargs<'a: 'a>(keybuf: &'a mut String, name: &'a str, args: &[f32]) -> &'a str {
    if args.is_empty() {
        name
    } else {
        keybuf.clear();
        keybuf.reserve(name.len() + args.len() * 20);
        keybuf.push_str(name);
        for f in args {
            keybuf.push_str(" , ");
            keybuf.push_str(&f.to_string());
        }
        keybuf.as_str()
    }
}

/// Type alias for `BTreeMap<String,f32>`
pub type StringTof32Namespace = BTreeMap<String, f32>;
impl EvalNamespace for StringTof32Namespace {
    #[inline]
    fn lookup(&mut self, name: &str, args: Vec<f32>, keybuf: &mut String) -> Option<f32> {
        let key = key_from_nameargs(keybuf, name, &args);
        self.get(key).copied()
    }
}

/// Type alias for `BTreeMap<&'static str,f32>`
pub type StrTof32Namespace = BTreeMap<&'static str, f32>;
impl EvalNamespace for StrTof32Namespace {
    #[inline]
    fn lookup(&mut self, name: &str, args: Vec<f32>, keybuf: &mut String) -> Option<f32> {
        let key = key_from_nameargs(keybuf, name, &args);
        self.get(key).copied()
    }
}

/// Type alias for `BTreeMap<String, Box<dyn FnMut(Vec<f32>)->f32>>`
///
/// This namespace type provides a very convenient way to register variables
/// and custom functions.  It is a bit slower than a pure callback, but it has
/// isolation and composition advantages.
pub type StringToCallbackNamespace<'a> = BTreeMap<String, Box<dyn FnMut(Vec<f32>) -> f32 + 'a>>;
impl EvalNamespace for StringToCallbackNamespace<'_> {
    #[inline]
    fn lookup(&mut self, name: &str, args: Vec<f32>, _keybuf: &mut String) -> Option<f32> {
        self.get_mut(name).map(|function| function(args))
        /*if let Some(f) = self.get_mut(name) {
            Some(f(args))
        } else {
            None
        }*/
    }
}

/// Type alias for `BTreeMap<&'static str, Box<dyn FnMut(Vec<f32>)->f32>>`
///
/// This namespace type provides a very convenient way to register variables
/// and custom functions.  It is a bit slower than a pure callback, but it has
/// isolation and composition advantages.
pub type StrToCallbackNamespace<'a> = BTreeMap<&'static str, Box<dyn FnMut(Vec<f32>) -> f32 + 'a>>;
impl EvalNamespace for StrToCallbackNamespace<'_> {
    #[inline]
    fn lookup(&mut self, name: &str, args: Vec<f32>, _keybuf: &mut String) -> Option<f32> {
        self.get_mut(name).map(|function| function(args))
        /*if let Some(f) = self.get_mut(name) {
            Some(f(args))
        } else {
            None
        }*/
    }
}

/// Type alias for `Vec<BTreeMap<String,f32>>`
pub type LayeredStringTof32Namespace = Vec<BTreeMap<String, f32>>;
impl EvalNamespace for LayeredStringTof32Namespace {
    #[inline]
    fn lookup(&mut self, name: &str, args: Vec<f32>, keybuf: &mut String) -> Option<f32> {
        let key = key_from_nameargs(keybuf, name, &args);

        for map in self.iter().rev() {
            if let Some(&val) = map.get(key) {
                return Some(val);
            }
        }
        None
    }
}

// I'm not making a type alias for this because of the un-name-ability of closures:
impl<F> EvalNamespace for F
where
    F: FnMut(&str, Vec<f32>) -> Option<f32>,
{
    #[inline]
    fn lookup(&mut self, name: &str, args: Vec<f32>, _keybuf: &mut String) -> Option<f32> {
        self(name, args)
    }
}

impl EvalNamespace for EmptyNamespace {
    /// Always returns `None`, indicating that the variable is undefined.
    #[inline]
    fn lookup(&mut self, _name: &str, _args: Vec<f32>, _keybuf: &mut String) -> Option<f32> {
        None
    }
}

impl EvalNamespace for CachedCallbackNamespace<'_> {
    /// Returns a cached value if possible, otherwise delegates to the callback function.
    fn lookup(&mut self, name: &str, args: Vec<f32>, keybuf: &mut String) -> Option<f32> {
        let key = key_from_nameargs(keybuf, name, &args);

        if let Some(&val) = self.cache.get(key) {
            return Some(val);
        }

        match (self.cb)(name, args) {
            Some(val) => {
                self.cache.insert(key.to_owned(), val);
                Some(val)
            }
            None => None,
        }
    }
}
impl Cached for CachedCallbackNamespace<'_> {
    fn cache_create(&mut self, name: String, val: f32) -> Result<(), Error> {
        if self.cache.contains_key(&name) {
            return Err(Error::AlreadyExists);
        }
        self.cache.insert(name, val);
        Ok(())
    }
    fn cache_set(&mut self, name: String, val: f32) {
        self.cache.insert(name, val);
    }
    fn cache_clear(&mut self) {
        self.cache = BTreeMap::new();
    }
}
impl<'a> CachedCallbackNamespace<'a> {
    #[inline]
    pub fn new<F>(cb: F) -> Self
    where
        F: FnMut(&str, Vec<f32>) -> Option<f32> + 'a,
    {
        CachedCallbackNamespace {
            cache: BTreeMap::new(),
            cb: Box::new(cb),
        }
    }
}

//// I am not ready to make this part of the public API yet.
// impl EvalNamespace for CachedLayeredNamespace<'_> {
//     fn lookup(&mut self, name:&str, args:Vec<f32>, keybuf:&mut String) -> Option<f32> {
//         let key = key_from_nameargs(keybuf, name, &args);
//
//         for map in self.caches.iter().rev() {
//             if let Some(&val) = map.get(key) { return Some(val); }
//         }
//
//         match (self.cb)(name,args) {
//             Some(val) => {
//                 // I'm using this panic-free 'match' structure for performance:
//                 match self.caches.last_mut() {
//                     Some(m_ref) => { m_ref.insert(key.to_string(),val); }
//                     None => (),  // unreachable
//                 }
//                 Some(val)
//             }
//             None => None,
//         }
//     }
//     fn cache_create(&mut self, name:String, val:f32) -> Result<(),Error> {
//         match self.caches.last_mut() {
//             Some(cur_layer) => {
//                 if cur_layer.contains_key(&name) { return Err(Error::AlreadyExists); }
//                 cur_layer.insert(name, val);
//             }
//             None => return Err(Error::Unreachable),
//         };
//         Ok(())
//     }
//     fn cache_set(&mut self, name:String, val:f32) {
//         match self.caches.last_mut() {
//             Some(m_ref) => { m_ref.insert(name, val); }
//             None => (),  // unreachable
//         }
//     }
//     fn cache_clear(&mut self) {
//         self.caches = Vec::with_capacity(self.caches.len());  // Assume the future usage will be similar to historical usage.
//         self.push();
//     }
// }
// impl Layered for CachedLayeredNamespace<'_> {
//     #[inline]
//     fn push(&mut self) {
//         self.caches.push(BTreeMap::new());
//     }
//     #[inline]
//     fn pop(&mut self) {
//         self.caches.pop();
//     }
// }
// impl<'a> CachedLayeredNamespace<'a> {
//     #[inline]
//     pub fn new<F>(cb:F) -> Self where F:FnMut(&str,Vec<f32>)->Option<f32> + 'a {
//         let mut ns = CachedLayeredNamespace{
//             caches:Vec::with_capacity(2),
//             cb    :Box::new(cb),
//         };
//         ns.push();
//         ns
//     }
// }

//// I am not ready to make this part of the public API yet.
// impl<NS> Bubble<'_,NS> where NS:EvalNamespace+Layered {
//     pub fn new<'a>(ns:&'a mut NS) -> Bubble<'a,NS> {
//         Bubble{
//             ns,
//             count:0,
//         }
//     }
// }
// impl<NS> Drop for Bubble<'_,NS> where NS:EvalNamespace+Layered {
//     fn drop(&mut self) {
//         while self.count>0 {
//             self.pop();
//         }
//     }
// }
// impl<NS> EvalNamespace for Bubble<'_,NS> where NS:EvalNamespace+Layered {
//     #[inline]
//     fn lookup(&mut self, name:&str, args:Vec<f32>, keybuf:&mut String) -> Option<f32> {
//         self.ns.lookup(name,args,keybuf)
//     }
//     #[inline]
//     fn cache_create(&mut self, name:String, val:f32) -> Result<(),Error> {
//         self.ns.cache_create(name,val)
//     }
//     #[inline]
//     fn cache_set(&mut self, name:String, val:f32) {
//         self.ns.cache_set(name,val)
//     }
//     #[inline]
//     fn cache_clear(&mut self) {
//         self.ns.cache_clear()
//     }
// }
// impl<NS> Layered for Bubble<'_,NS> where NS:EvalNamespace+Layered {
//     #[inline]
//     fn push(&mut self) {
//         self.ns.push();
//         self.count = self.count+1;
//     }
//     #[inline]
//     fn pop(&mut self) {
//         if self.count>0 {
//             self.count = self.count+1;
//             self.ns.pop();
//         }
//     }
// }

//// Commented out until we start using a layered namespace again.
// #[cfg(test)]
// mod internal_tests {
//     use super::*;
//
//     #[test]
//     fn bubble() {
//         let mut ns = CachedLayeredNamespace::new(|_,_| None);
//         assert_eq!(ns.caches.len(), 1);
//         {
//             let mut bub = Bubble::new(&mut ns);  bub.push();
//             assert_eq!(bub.ns.caches.len(), 2);
//             bub.push();
//             assert_eq!(bub.ns.caches.len(), 3);
//             bub.push();
//             assert_eq!(bub.ns.caches.len(), 4);
//             bub.pop();
//             assert_eq!(bub.ns.caches.len(), 3);
//         }
//         assert_eq!(ns.caches.len(), 1);
//     }
// }
