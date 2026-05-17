//! FP-Class-A guard: keep the "DEAD CODE" labels honest.
//!
//! Several byte-verified solvers are documented `DEAD CODE` because they
//! have **zero non-test callers** (the live path is in-V8 self-solve).
//! This test fails if any of them gains a reference from a non-test
//! source file — forcing the label (and the engine docs) to be updated
//! deliberately rather than silently drifting back into the
//! "exists ≠ exercised" false-positive class.
//!
//! Heuristic (intentionally conservative): a symbol is "still dead" iff
//! every `crates/**/*.rs` file that mentions it is either its own
//! defining file or lives under a `tests/` directory. A new non-test
//! file referencing it ⇒ failure with a pointer to update the label.

use std::path::PathBuf;
use std::process::Command;

fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR = <root>/crates/akamai
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf()
}

/// (symbol, defining file relative to crates/, human note)
const DEAD_SYMBOLS: &[(&str, &str, &str)] = &[
    ("solve_crypto", "akamai/src/sec_cpt.rs", "sec-cpt PoW solver"),
    ("DdEncryptor", "akamai/src/datadome_crypto.rs", "DataDome interstitial encryptor"),
    ("BotScoreVector", "akamai/src/lib.rs", "ak_p server-timing bot score"),
];

#[test]
fn dead_labeled_symbols_have_no_non_test_callers() {
    let root = workspace_root();
    let crates_dir = root.join("crates");
    let mut violations = Vec::new();

    for (symbol, def_rel, note) in DEAD_SYMBOLS {
        // List every .rs file under crates/ that mentions the symbol.
        let out = Command::new("grep")
            .args(["-rl", "--include=*.rs", symbol])
            .arg(&crates_dir)
            .output()
            .expect("grep runs");
        let files = String::from_utf8_lossy(&out.stdout);
        for f in files.lines().filter(|l| !l.is_empty()) {
            let is_def = f.ends_with(def_rel);
            let is_test = f.contains("/tests/") || f.ends_with("dead_code_labels.rs");
            if !is_def && !is_test {
                violations.push(format!(
                    "`{symbol}` ({note}) is referenced by NON-TEST file `{f}` \
                     but is labelled DEAD CODE. Either the label is now wrong \
                     (update the doc-comment + this list + the engine docs \
                     FP-Class-A) or that reference belongs in a test.",
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "FP-Class-A dead-code label drift:\n{}",
        violations.join("\n")
    );
}
