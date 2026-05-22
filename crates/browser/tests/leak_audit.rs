use browser::Page;
use stealth::presets;

#[tokio::test]
async fn audit_global_this_leaks() {
    let profile = presets::chrome_148_ru();

    // We use a blank page but our bootstrap scripts will run
    let mut page: Page = Page::with_profile("", "about:blank", profile)
        .await
        .unwrap();

    let audit_script = r#"
        (function() {
            function getProps(obj) {
                const props = [];
                for (let prop in obj) {
                    props.push(prop);
                }
                return props.sort();
            }

            return JSON.stringify({
                window: getProps(window),
                navigator: getProps(navigator),
                document: getProps(document),
                screen: getProps(screen),
                navigatorValues: {
                    userAgent: navigator.userAgent,
                    appName: navigator.appName,
                    appCodeName: navigator.appCodeName,
                    vendor: navigator.vendor,
                    platform: navigator.platform,
                    cookieEnabled: navigator.cookieEnabled
                },
                denoExists: typeof Deno !== 'undefined',
                opsExists: typeof Deno !== 'undefined' && typeof Deno.core !== 'undefined' && typeof Deno.core.ops !== 'undefined'
            });
        })()
    "#;

    let result: String = page.evaluate(audit_script).unwrap();
    println!("Global Audit Result: {}", result);

    // Basic assertions to catch obvious leaks
    let result_json: serde_json::Value = serde_json::from_str(&result).unwrap();

    if result_json["denoExists"].as_bool().unwrap_or(false) {
        println!("FAIL: Deno global is leaked!");
    }

    if result_json["opsExists"].as_bool().unwrap_or(false) {
        println!("FAIL: Deno.core.ops is leaked!");
    }
}

#[tokio::test]
async fn audit_function_to_string_leaks() {
    let profile = presets::chrome_148_ru();

    let mut page: Page = Page::with_profile("", "about:blank", profile)
        .await
        .unwrap();

    // Check toString of some standard properties that we might have patched or backed with ops
    let check_script = r#"
        (function() {
            const targets = [
                'navigator.userAgent',
                'navigator.webdriver',
                'navigator.languages',
                'CanvasRenderingContext2D.prototype.fillRect',
                'HTMLCanvasElement.prototype.getContext',
                'OfflineAudioContext.prototype.startRendering'
            ];

            const results = {};
            for (const path of targets) {
                try {
                    const parts = path.split('.');
                    const prop = parts.pop();
                    const objPath = parts.join('.');
                    const obj = objPath ? eval(objPath) : window;
                    
                    const desc = Object.getOwnPropertyDescriptor(obj, prop) || 
                                 (obj.prototype ? Object.getOwnPropertyDescriptor(obj.prototype, prop) : null) ||
                                 Object.getOwnPropertyDescriptor(Object.getPrototypeOf(obj), prop);
                    
                    if (desc) {
                        if (desc.get) {
                            results[path + ' (getter)'] = desc.get.toString();
                        }
                        if (typeof desc.value === 'function') {
                            results[path] = desc.value.toString();
                        } else if (desc.value !== undefined) {
                            results[path] = 'value: ' + desc.value;
                        }
                    } else {
                        const val = eval(path);
                        if (typeof val === 'function') {
                            results[path] = val.toString();
                        } else {
                            results[path] = 'not found and not a function';
                        }
                    }
                } catch (e) {
                    results[path] = 'error: ' + e.message;
                }
            }
            return JSON.stringify(results);
        })()
    "#;

    let result: String = page.evaluate(check_script).unwrap();
    println!("Function toString Audit Result: {}", result);
}

// §6 item 7 — navigator.permissions.query must return a per-name state map
// matching headed Chrome 133. A blanket "prompt" (or the headless "denied"
// for notifications) is the classic automation tell.
#[tokio::test]
async fn test_permissions_query_defaults() {
    let profile = presets::chrome_148_windows();
    // Notifications/clipboard/etc. are [SecureContext]-gated; about:blank
    // would return "denied" uniformly. Real Chrome 133's per-name map
    // (the reference here) was captured on a secure context.
    let mut page: Page = Page::with_profile("", "https://example.com/", profile)
        .await
        .unwrap();

    let probe = r#"
        (async () => {
            const names = [
                'notifications', 'geolocation', 'camera', 'microphone', 'midi',
                'persistent-storage', 'background-sync', 'clipboard-write',
                'accelerometer', 'gyroscope', 'magnetometer', 'screen-wake-lock'
            ];
            const out = {};
            for (const name of names) {
                try {
                    const r = await navigator.permissions.query({ name });
                    out[name] = r.state;
                } catch (e) {
                    out[name] = 'THREW:' + e.message;
                }
            }
            // invalid name must reject with TypeError
            try {
                await navigator.permissions.query({ name: 'not-a-real-permission' });
                out['_invalid'] = 'DID-NOT-THROW';
            } catch (e) {
                out['_invalid'] = e.constructor.name;
            }
            // missing descriptor must reject
            try {
                await navigator.permissions.query();
                out['_missing'] = 'DID-NOT-THROW';
            } catch (e) {
                out['_missing'] = e.constructor.name;
            }
            globalThis.__permResult = JSON.stringify(out);
        })()
    "#;
    page.evaluate(probe).unwrap();
    page.evaluate_async("void 0", std::time::Duration::from_millis(100))
        .await
        .ok();
    let raw = page.evaluate("globalThis.__permResult").unwrap();
    let obj: serde_json::Value = serde_json::from_str(&raw)
        .unwrap_or_else(|_| panic!("permissions probe result was not JSON: {}", raw));

    // W3C PermissionState: granted | denied | prompt. Never 'default'.
    assert_eq!(
        obj["notifications"], "prompt",
        "headless 'denied' is a classic tell"
    );
    assert_eq!(obj["geolocation"], "prompt");
    assert_eq!(obj["camera"], "prompt");
    assert_eq!(obj["microphone"], "prompt");
    assert_eq!(obj["midi"], "prompt");
    assert_eq!(obj["persistent-storage"], "granted");
    assert_eq!(obj["background-sync"], "granted");
    assert_eq!(obj["clipboard-write"], "granted");
    assert_eq!(obj["accelerometer"], "granted");
    assert_eq!(obj["gyroscope"], "granted");
    assert_eq!(obj["magnetometer"], "granted");
    assert_eq!(obj["screen-wake-lock"], "granted");
    // Chrome rejects invalid permission names with TypeError
    assert_eq!(obj["_invalid"], "TypeError");
    assert_eq!(obj["_missing"], "TypeError");
}
