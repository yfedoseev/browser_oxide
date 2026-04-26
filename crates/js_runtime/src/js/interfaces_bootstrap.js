/**
 * Interface bootstrap — defines standard Web IDL classes.
 * Runs FIRST to ensure these globals are available to all other scripts.
 */
((globalThis) => {
    function _define(name, cls) {
        if (globalThis[name]) {
            return;
        }
        Object.defineProperty(cls.prototype, Symbol.toStringTag, {
            value: name, configurable: true
        });
        Object.defineProperty(globalThis, name, {
            value: cls, configurable: true, writable: true, enumerable: false
        });
    }

    _define("Navigator", class Navigator {});
    _define("Location", class Location {});
    _define("History", class History {});
    _define("Screen", class Screen {});
    _define("EventTarget", class EventTarget {});
    _define("Event", class Event { constructor(type, init) { this.type = type; } });
    _define("MessageEvent", class MessageEvent extends (globalThis.Event || class {}) {});
    _define("CustomEvent", class CustomEvent extends (globalThis.Event || class {}) {});
    _define("Performance", class Performance {});
    _define("PluginArray", class PluginArray {});
    _define("MimeTypeArray", class MimeTypeArray {});
    _define("Plugin", class Plugin {});
    _define("MimeType", class MimeType {});
    _define("NetworkInformation", class NetworkInformation {});
    _define("MediaDevices", class MediaDevices {});
    _define("StorageManager", class StorageManager {});
    _define("Bluetooth", class Bluetooth {});
    _define("PermissionStatus", class PermissionStatus {});
    _define("Permissions", class Permissions {});
    _define("ScreenOrientation", class ScreenOrientation {});

    // WebGL Constants
    globalThis.WebGLRenderingContext = globalThis.WebGLRenderingContext || {
        UNMASKED_VENDOR_WEBGL: 0x9245,
        UNMASKED_RENDERER_WEBGL: 0x9246,
    };
    globalThis.WebGL2RenderingContext = globalThis.WebGL2RenderingContext || {
        UNMASKED_VENDOR_WEBGL: 0x9245,
        UNMASKED_RENDERER_WEBGL: 0x9246,
    };

    // Common non-standard Chrome global
    if (!globalThis.chrome) {
        globalThis.chrome = {
            app: { isInstalled: false },
            runtime: { OnInstalledReason: { INSTALL: "install", UPDATE: "update", CHROME_UPDATE: "chrome_update", SHARED_MODULE_UPDATE: "shared_module_update" } }
        };
    }

    // Common modern APIs
    if (!globalThis.requestIdleCallback) {
        globalThis.requestIdleCallback = function(cb) {
            return setTimeout(() => {
                cb({ didTimeout: false, timeRemaining: () => 10 });
            }, 1);
        };
        globalThis.cancelIdleCallback = function(id) {
            clearTimeout(id);
        };
    }

    // __errors is an internal buffer for challenge debugging. Must not
    // leak to page scripts — a site that does `Object.keys(window)`
    // would see it and flag us. Kept non-enumerable and deleted by
    // cleanup_bootstrap.js.
    Object.defineProperty(globalThis, '__errors', {
        value: [], enumerable: false, configurable: true, writable: true,
    });
    globalThis.onerror = function(msg, url, line, col, error) {
        globalThis.__errors.push({
            msg: String(msg),
            url: String(url),
            line: line,
            col: col,
            stack: error ? String(error.stack) : ""
        });
        return false;
    };

})(globalThis);
