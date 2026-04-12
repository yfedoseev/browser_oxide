/**
 * Instance bootstrap — creates the base instances of standard browser objects.
 * Runs early so that subsequent bootstraps (like dom) can reference them.
 */
((globalThis) => {
    if (!globalThis.navigator) {
        globalThis.navigator = Object.create(globalThis.Navigator.prototype);
    }
    if (!globalThis.location) {
        globalThis.location = Object.create(globalThis.Location.prototype);
    }
    if (!globalThis.history) {
        globalThis.history = Object.create(globalThis.History.prototype);
    }
    if (!globalThis.screen) {
        globalThis.screen = Object.create(globalThis.Screen.prototype);
    }
    if (!globalThis.performance) {
        globalThis.performance = Object.create(globalThis.Performance.prototype);
    }
})(globalThis);
