// Human-like input simulation API
// Used by CDP Input.dispatchMouseEvent / Input.dispatchKeyEvent
// and directly via page.humanClick() / page.humanType()
((globalThis) => {
    globalThis.__browserOxide = globalThis.__browserOxide || {};

    // Generate a human-like mouse movement path
    // Returns [{x, y, delay_ms}, ...]
    globalThis.__browserOxide.humanMousePath = function(x1, y1, x2, y2, steps) {
        return Deno.core.ops.op_human_mouse_path(x1, y1, x2, y2, steps || 20);
    };

    // Generate human-like typing delays for a string
    // Returns [delay_ms, delay_ms, ...] (one per character)
    globalThis.__browserOxide.humanTypingDelays = function(text, wpm) {
        return Deno.core.ops.op_human_typing_delays(text, wpm || 65);
    };
})(globalThis);
