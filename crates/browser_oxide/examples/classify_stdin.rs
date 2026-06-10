//! Benchmark helper: read an HTML body from stdin, print the canonical
//! `engine_classify` tag + byte length. Lets the competitor benchmark
//! harness scores every browser's final
//! rendered DOM through the *exact same* classifier browser_oxide uses
//! for its own corpus sweep, so the comparison has zero classifier drift.
//!
//! Output: one line `<tag>\t<len>` on stdout.

use std::io::Read;

fn main() {
    let mut body = String::new();
    std::io::stdin()
        .read_to_string(&mut body)
        .expect("read stdin");
    let ec = browser_oxide::engine_classify(&body);
    println!("{}\t{}", ec.tag, ec.len);
}
