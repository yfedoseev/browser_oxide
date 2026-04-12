use browser::Page;
use stealth::presets;

#[tokio::test]
async fn audit_global_this_leaks() {
    let profile = presets::chrome_130_ru();

    // We use a blank page but our bootstrap scripts will run
    let mut page: Page = Page::with_profile("", "about:blank", profile).await.unwrap();
    
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
    let profile = presets::chrome_130_ru();

    let mut page: Page = Page::with_profile("", "about:blank", profile).await.unwrap();
    
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
