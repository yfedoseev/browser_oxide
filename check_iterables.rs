
async fn main() {
    let js = r#"
        (function() {
            const targets = {
                'navigator.plugins': navigator.plugins,
                'navigator.mimeTypes': navigator.mimeTypes,
                'document.fonts': document.fonts,
                'document.styleSheets': document.styleSheets,
                'document.scripts': document.scripts,
                'document.all': document.all,
                'window.frames': window.frames,
            };
            if (navigator.userAgentData) {
                targets['navigator.userAgentData.brands'] = navigator.userAgentData.brands;
            }
            const results = {};
            for (const [name, obj] of Object.entries(targets)) {
                try {
                    if (obj === undefined) {
                        results[name] = 'undefined';
                    } else if (obj === null) {
                        results[name] = 'null';
                    } else {
                        results[name] = typeof obj[Symbol.iterator] === 'function' ? 'ITERABLE' : 'NOT_ITERABLE';
                    }
                } catch (e) {
                    results[name] = 'ERROR: ' + e.message;
                }
            }
            return JSON.stringify(results, null, 2);
        })()
    "#;
    // ... I'll run this in a test ...
}
