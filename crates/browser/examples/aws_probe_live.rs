//! parity-workflows task #21 — live AWS-WAF self-solve probe.
//!
//! Navigates a live AWS-WAF-challenged URL with an instrumentation
//! init_script that wraps `Worker`, `URL.createObjectURL`, and
//! `AwsWafIntegration.{checkForceRefresh,getToken,forceRefreshToken,saveReferrer}`,
//! and captures window.onerror / unhandledrejection. After the nav it dumps
//! the captured log so we can see EXACTLY where challenge.js's self-solve
//! bails (which branch runs, whether the promise resolves/rejects/throws,
//! whether a blob worker is ever created).
//!
//! Usage:
//!   aws_probe_live <url> [profile] [iterations]
//!
//! Established 2026-05-28 that the live nav emits zero op_blob_register /
//! op_worker_spawn despite challenge.js loading — this probe localizes WHY.

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let mut args = std::env::args().skip(1);
    let url = args.next().unwrap_or_else(|| {
        eprintln!("usage: aws_probe_live <url> [profile] [iterations]");
        std::process::exit(2);
    });
    let profile_name = args.next().unwrap_or_else(|| "chrome_148_macos".into());
    let iterations: u8 = args
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2);
    let profile = match profile_name.as_str() {
        "chrome_148_macos" => stealth::presets::chrome_148_macos(),
        "chrome_148_windows" => stealth::presets::chrome_148_windows(),
        "firefox_135_macos" => stealth::presets::firefox_135_macos(),
        "iphone_15_pro_safari_18" => stealth::presets::iphone_15_pro_safari_18(),
        other => {
            eprintln!("unknown profile {other}");
            std::process::exit(2);
        }
    };

    // Instrumentation init_script — runs on every iteration's fresh runtime
    // BEFORE the page's own scripts (so it can trap challenge.js's calls).
    let probe = r#"
        globalThis.__awsProbe = { errors: [], log: [] };
        const _push = (s) => { try { globalThis.__awsProbe.log.push(String(s)); } catch(_){} };
        try {
            globalThis.addEventListener && globalThis.addEventListener('error', (e) => {
                globalThis.__awsProbe.errors.push('onerror: ' + String(e && (e.message||e)) +
                    (e && e.error && e.error.stack ? ('\n' + e.error.stack) : ''));
            });
            globalThis.addEventListener && globalThis.addEventListener('unhandledrejection', (e) => {
                const r = e && e.reason;
                globalThis.__awsProbe.errors.push('unhandledrejection: ' +
                    String(r && (r.stack || r.message || r)));
            });
        } catch(_){}
        try {
            const _of = globalThis.fetch;
            if (_of) {
                globalThis.fetch = function(u, o) {
                    const _u = (u && u.url) ? u.url : String(u);
                    _push('fetch(' + _u.slice(0,90) + ' method=' + ((o&&o.method)||'GET') + ')');
                    const p = _of.apply(this, arguments);
                    p.then((r) => _push('fetch DONE ' + _u.slice(0,60) + ' status=' + (r && r.status)),
                           (e) => _push('fetch FAIL ' + _u.slice(0,60) + ' ' + String(e && (e.message||e))));
                    return p;
                };
            }
        } catch(_){}
        try {
            const _OW = globalThis.Worker;
            if (_OW) {
                globalThis.Worker = function(u, o) {
                    _push('new Worker(' + String(u).slice(0,90) + ')');
                    return new _OW(u, o);
                };
                globalThis.Worker.prototype = _OW.prototype;
            }
        } catch(_){}
        try {
            if (globalThis.URL && URL.createObjectURL) {
                const _co = URL.createObjectURL;
                URL.createObjectURL = function(obj) {
                    const r = _co.apply(this, arguments);
                    _push('createObjectURL -> ' + String(r).slice(0,60) +
                        ' (type=' + (obj && obj.type) + ' size=' + (obj && obj.size) + ')');
                    return r;
                };
            }
        } catch(_){}
        // Trap AwsWafIntegration assignment + wrap its async methods.
        try {
            let _awi;
            const _wrap = (v) => {
                if (!v || typeof v !== 'object') return v;
                for (const m of ['checkForceRefresh','getToken','forceRefreshToken','saveReferrer']) {
                    if (typeof v[m] !== 'function') continue;
                    const _orig = v[m];
                    v[m] = function() {
                        _push('CALL AwsWafIntegration.' + m);
                        try {
                            const r = _orig.apply(this, arguments);
                            if (r && typeof r.then === 'function') {
                                r.then((x) => {
                                    _push(m + ' RESOLVED: ' + (()=>{try{return JSON.stringify(x)}catch(_){return String(x)}})());
                                    _push('  document.cookie after ' + m + ': ' + String(document.cookie).slice(0,200));
                                    try { _push('  hasToken()=' + (typeof _awi.hasToken === 'function' ? _awi.hasToken() : 'n/a')); } catch(_){}
                                    // DECISIVE: does a same-origin fetch now (token in jar) return real content?
                                    if (m === 'forceRefreshToken' || m === 'getToken') {
                                        setTimeout(() => {
                                            try {
                                                fetch(location.href, { credentials: 'include' })
                                                    .then((rr) => rr.text().then((tt) => _push('POST-SOLVE refetch ' + m + ': status=' + rr.status + ' len=' + tt.length)))
                                                    .catch((e2) => _push('POST-SOLVE refetch ERR: ' + String(e2 && (e2.message||e2))));
                                            } catch (e3) { _push('POST-SOLVE refetch THREW: ' + String(e3)); }
                                        }, 600);
                                    }
                                }, (e) => _push(m + ' REJECTED: ' + String(e && (e.stack||e.message||e))));
                            }
                            return r;
                        } catch (e) {
                            _push(m + ' THREW: ' + String(e && (e.stack||e.message||e)));
                            throw e;
                        }
                    };
                }
                return v;
            };
            Object.defineProperty(globalThis, 'AwsWafIntegration', {
                configurable: true,
                get() { return _awi; },
                set(v) {
                    _push('AwsWafIntegration assigned: keys=' + (v ? Object.keys(v).join(',') : 'null'));
                    _awi = _wrap(v);
                },
            });
        } catch(e) { _push('trap-install-error: ' + String(e)); }
    "#;

    eprintln!("[aws_probe_live] navigating {url} (profile {profile_name}, {iterations} iters)");
    let profile_for_check = profile.clone();
    match browser::Page::navigate_with_init(&url, profile, iterations, vec![probe.to_string()]).await
    {
        Ok(mut page) => {
            let body_len = page.content().len();
            let final_cookie = page
                .evaluate("String(document.cookie)")
                .unwrap_or_default();
            let dump = page
                .evaluate("JSON.stringify(globalThis.__awsProbe || {note:'no probe state'}, null, 1)")
                .unwrap_or_else(|e| format!("{{\"evaluate_error\":\"{e}\"}}"));
            eprintln!("[aws_probe_live] final body_len={body_len}");
            eprintln!("[aws_probe_live] final document.cookie: {final_cookie}");
            println!("{dump}");

            // DECISIVE: the process-wide shared jar is the one the nav used.
            // Does it now carry aws-waf-token, and does a fresh Rust GET (which
            // sends the jar) return real content? This separates a navigate-loop
            // gap (token works, loop didn't retry) from a token-rejection.
            if let Ok(client) = net::HttpClient::shared(&profile_for_check) {
                if let Ok(parsed) = url::Url::parse(&url) {
                    let jar = client.cookies_for_url(&parsed).await.unwrap_or_default();
                    eprintln!(
                        "[aws_probe_live] shared-jar has aws-waf-token: {} | jar='{}'",
                        jar.contains("aws-waf-token"),
                        jar.chars().take(160).collect::<String>()
                    );
                }
                match client.get_follow(&url, 10).await {
                    Ok(resp) => eprintln!(
                        "[aws_probe_live] post-solve Rust GET: status={} len={}",
                        resp.status,
                        resp.text().len()
                    ),
                    Err(e) => eprintln!("[aws_probe_live] post-solve Rust GET err: {e}"),
                }
            }
        }
        Err(e) => {
            eprintln!("[aws_probe_live] navigate error: {e}");
            std::process::exit(1);
        }
    }
}
