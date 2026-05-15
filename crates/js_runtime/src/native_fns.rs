//! Genuine-native host functions built from raw `v8::FunctionTemplate`
//! (the same primitive deno_core uses for every `#[op2]`,
//! `deno_core-0.311.0/runtime/bindings.rs:368`).
//!
//! WHY: V8 builds the `class X extends <fn> {}` TypeError message,
//! `Object::NoSideEffectsToString`, error stacks and `eval`-stringify
//! from a function's internal `[[SourceText]]` via `JSFunction::ToString`,
//! which emits `function NAME() { [native code] }` ONLY when
//! `!SharedFunctionInfo::IsUserJavaScript()` — true exactly for API
//! functions (FunctionTemplate, `script()==undefined`). A JS-level
//! `Function.prototype.toString` patch (stealth_bootstrap.js) CANNOT
//! intercept these internal stringifiers, so our JS shims leak source
//! (Kasada `fsc` probe: `class K extends Function.prototype.toString{}`
//! leaked `_patchedFnToStr` source — doc 27 §3, primary V8 source).
//!
//! This installs `Function.prototype.toString` as a genuine API
//! function. Behaviour preserved: masked host fns (carrying the
//! `Symbol.for('__boxide_native__')` tag set by stealth_bootstrap.js)
//! stringify as `function <tag>() { [native code] }`; everything else
//! delegates to the GENUINE original `Function.prototype.toString`
//! (captured BEFORE any bootstrap ran) so real JS user functions still
//! return their source and real natives return `[native code]` — exactly
//! as V8 would. Because the installed function is itself an API
//! function it is non-constructable, `.prototype`-less, and source-less:
//! the class-extends / NoSideEffectsToString leak is structurally gone.

use deno_core::v8;
use std::collections::HashMap;

const NATIVE_TAG: &str = "__boxide_native__";

/// Per-runtime storage for child iframe realms (genuine v8::Context instances).
///
/// Each entry keeps the child `v8::Context` alive (without a Global it would
/// be GC'd) and caches the child global object to avoid re-creating on
/// successive `contentWindow` reads from JS. Keyed by a monotonically-
/// increasing realm ID assigned by `_nextRealmId` in dom_bootstrap.js.
///
/// `orig_fp_tostring` is the builtin `Function.prototype.toString` captured
/// pre-bootstrap; it is installed into every child realm so cross-realm
/// `toString` calls (`cw.Function.prototype.toString.call(parent.fetch)`)
/// produce `[native code]` — same as the main window.
pub struct IframeRealmStore {
    pub contexts: HashMap<u32, v8::Global<v8::Context>>,
    pub globals: HashMap<u32, v8::Global<v8::Object>>,
    pub orig_fp_tostring: Option<v8::Global<v8::Function>>,
}

impl IframeRealmStore {
    pub fn new() -> Self {
        Self {
            contexts: HashMap::new(),
            globals: HashMap::new(),
            orig_fp_tostring: None,
        }
    }
}

/// Create a GENUINE child realm — a second `v8::Context` in the same
/// isolate, exactly what Chrome does per iframe (doc 27 §1; the
/// primitive deno_core itself uses, `jsruntime.rs:2048`). The child
/// gets its OWN full set of native builtins for free
/// (`Object`/`Function`/`Array`/`Reflect`/`Symbol`/`Map`/`TypeError`/…
/// — the exact 0x12 list Kasada's VM reads, doc 26), each genuinely
/// `[native code]` and realm-distinct from the parent — i.e. NOT a
/// `Proxy` (defeats Kasada's named `addContentWindowProxy` detector)
/// and NOT parent-aliased (defeats the realm-divergence bail).
///
/// This is the PRIMITIVE only (additive, not yet wired into
/// `_getIframeWindow`) — proves the core mechanism works in our
/// single-isolate deno_core engine before the larger wiring pass.
/// Returns the child context's global object as a Global so callers
/// can hold it as `iframe.contentWindow` and keep the context alive.
pub fn create_child_realm(
    scope: &mut v8::HandleScope,
) -> Option<(v8::Global<v8::Context>, v8::Global<v8::Object>)> {
    // A fresh context. Default options: the child gets the full native
    // intrinsic set + its own global. (Per doc 27 §1.5, identity probes
    // test builtins/prototype-chains/ctor-names — all native here;
    // DOM host shims are a later delegation step.)
    let ctx = v8::Context::new(scope, v8::ContextOptions::default());
    let global = {
        let cscope = &mut v8::ContextScope::new(scope, ctx);
        let g = ctx.global(cscope);
        v8::Global::new(cscope, g)
    };
    Some((v8::Global::new(scope, ctx), global))
}

/// Capture the genuine builtin `Function.prototype.toString` BEFORE any
/// bootstrap replaces it. Returned Global is passed as the API
/// function's `data` so untagged functions delegate to real V8
/// semantics. Call right after `JsRuntime::new`, before bootstrap.
pub fn capture_original_fp_tostring(
    scope: &mut v8::HandleScope,
) -> Option<v8::Global<v8::Function>> {
    let ctx = scope.get_current_context();
    let global = ctx.global(scope);
    let fkey = v8::String::new(scope, "Function")?;
    let fctor = global.get(scope, fkey.into())?;
    let fctor = v8::Local::<v8::Object>::try_from(fctor).ok()?;
    let pkey = v8::String::new(scope, "prototype")?;
    let fproto = fctor.get(scope, pkey.into())?;
    let fproto = v8::Local::<v8::Object>::try_from(fproto).ok()?;
    let tskey = v8::String::new(scope, "toString")?;
    let ts = fproto.get(scope, tskey.into())?;
    let ts = v8::Local::<v8::Function>::try_from(ts).ok()?;
    Some(v8::Global::new(scope, ts))
}

/// The genuine-native `Function.prototype.toString` callback.
/// `args.data()` carries the captured original (a `v8::Function`).
fn fp_to_string_cb(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    mut rv: v8::ReturnValue,
) {
    let this: v8::Local<v8::Value> = args.this().into();

    // Spec step 2: `this` must be callable, else TypeError (matches the
    // genuine builtin's `requires that 'this' be a Function`).
    if !this.is_function() {
        let msg = v8::String::new(
            scope,
            "Function.prototype.toString requires that 'this' be a Function",
        )
        .unwrap();
        let exc = v8::Exception::type_error(scope, msg);
        scope.throw_exception(exc);
        return;
    }
    let this_obj = v8::Local::<v8::Object>::try_from(this).unwrap();

    // Masked host fn? Read the global-registered native tag symbol.
    // (stealth_bootstrap sets `Symbol.for('__boxide_native__')` = name.)
    if let Some(key) = v8::String::new(scope, NATIVE_TAG) {
        let sym = v8::Symbol::for_global(scope, key);
        if let Some(tagv) = this_obj.get(scope, sym.into()) {
            if tagv.is_string() {
                let tag = tagv.to_rust_string_lossy(scope);
                let s = format!("function {tag}() {{ [native code] }}");
                if let Some(out) = v8::String::new(scope, &s) {
                    rv.set(out.into());
                    return;
                }
            }
        }
    }

    // Untagged: delegate to the GENUINE original Function.prototype
    // .toString (captured pre-bootstrap, passed via data). V8 itself
    // then emits `[native code]` for real natives and the real source
    // for genuine JS user functions — exactly correct.
    let data = args.data();
    if let Ok(orig) = v8::Local::<v8::Function>::try_from(data) {
        let recv = this;
        if let Some(res) = orig.call(scope, recv, &[]) {
            rv.set(res);
            return;
        }
    }
    // Last-resort fallback (should be unreachable): anonymous native.
    if let Some(out) =
        v8::String::new(scope, "function () { [native code] }")
    {
        rv.set(out.into());
    }
}

/// Install the genuine-native `Function.prototype.toString`, replacing
/// the JS-level patch. `original` must be the builtin captured pre-
/// bootstrap (see `capture_original_fp_tostring`). Run AFTER bootstrap
/// + cleanup, before site/init scripts.
pub fn install_native_fp_tostring(
    scope: &mut v8::HandleScope,
    original: &v8::Global<v8::Function>,
) -> bool {
    let orig_local = v8::Local::new(scope, original);

    let tmpl = v8::FunctionTemplate::builder(fp_to_string_cb)
        .length(0)
        .constructor_behavior(v8::ConstructorBehavior::Throw)
        .side_effect_type(v8::SideEffectType::HasNoSideEffect)
        .data(orig_local.into())
        .build(scope);
    if let Some(name) = v8::String::new(scope, "toString") {
        tmpl.set_class_name(name);
    }
    let func = match tmpl.get_function(scope) {
        Some(f) => f,
        None => return false,
    };
    if let Some(name) = v8::String::new(scope, "toString") {
        func.set_name(name);
    }

    // Install on Function.prototype with Chrome's attribute shape:
    // { value, writable:true, enumerable:false, configurable:true }.
    let ctx = scope.get_current_context();
    let global = ctx.global(scope);
    let Some(fkey) = v8::String::new(scope, "Function") else {
        return false;
    };
    let Some(fctor) = global.get(scope, fkey.into()) else {
        return false;
    };
    let Ok(fctor) = v8::Local::<v8::Object>::try_from(fctor) else {
        return false;
    };
    let Some(pkey) = v8::String::new(scope, "prototype") else {
        return false;
    };
    let Some(fproto) = fctor.get(scope, pkey.into()) else {
        return false;
    };
    let Ok(fproto) = v8::Local::<v8::Object>::try_from(fproto) else {
        return false;
    };
    let Some(tskey) = v8::String::new(scope, "toString") else {
        return false;
    };
    // define_own_property with DONT_ENUM (writable+configurable default).
    fproto.define_own_property(
        scope,
        tskey.into(),
        func.into(),
        v8::PropertyAttribute::DONT_ENUM,
    );
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use deno_core::{JsRuntime, RuntimeOptions};

    /// Verifies that `proxy.get_prototype()` returns the inner global (not
    /// Window.prototype), and that `create_data_property` on the inner global
    /// creates properties visible from INSIDE the child realm via script eval.
    /// Also tests that calling `set_prototype()` on the proxy (as done by
    /// `op_create_child_realm`) doesn't change what `get_prototype()` returns.
    #[test]
    fn verify_inner_global_property_visibility() {
        let mut rt = JsRuntime::new(RuntimeOptions::default());
        let scope = &mut rt.handle_scope();

        let child_ctx = v8::Context::new(scope, v8::ContextOptions::default());
        {
            let cs = &mut v8::ContextScope::new(scope, child_ctx);

            // Simulate what op_create_child_realm does: set a Window prototype
            let window_proto_src = v8::String::new(cs, "(function Window(){}).prototype").unwrap();
            let window_proto_script = v8::Script::compile(cs, window_proto_src, None).unwrap();
            let window_proto_val = window_proto_script.run(cs).unwrap();

            let proxy = child_ctx.global(cs);
            proxy.set_prototype(cs, window_proto_val); // mimic op_create_child_realm

            // Now get inner global via get_prototype (must still be inner global, not window_proto)
            let proto_after = proxy.get_prototype(cs).expect("proxy must have a prototype");
            let inner = v8::Local::<v8::Object>::try_from(proto_after)
                .expect("prototype after set_prototype must still be an Object (inner global)");

            // The inner global must NOT be the window_proto we set
            // (set_prototype sets inner_global's [[Prototype]], not the proxy's)
            let inner_hash = inner.get_identity_hash();
            let proxy_hash = proxy.get_identity_hash();
            assert_ne!(inner_hash, proxy_hash, "inner global must differ from proxy");

            let key = v8::String::new(cs, "__testProp__").unwrap();
            let val = v8::Integer::new(cs, 42);
            inner.create_data_property(cs, key.into(), val.into());

            // Property must be readable from inside the realm via eval
            let src = v8::String::new(cs, "typeof __testProp__ + ':' + __testProp__").unwrap();
            let script = v8::Script::compile(cs, src, None).unwrap();
            let res = script.run(cs).unwrap().to_rust_string_lossy(cs);
            assert_eq!(res, "number:42",
                "inner-global property must be visible inside realm after set_prototype; got: {res}");
        }
    }

    /// Proves the child-realm PRIMITIVE: a raw `v8::Context::new` child
    /// in the same isolate has its OWN genuine native intrinsics
    /// (`[native code]`, correct ctor names) and a global object
    /// distinct from the parent — i.e. exactly what Kasada wants from
    /// `iframe.contentWindow` (real realm, NOT a Proxy, NOT
    /// parent-aliased). Foundation for the _getIframeWindow wiring.
    #[test]
    fn child_realm_has_genuine_native_intrinsics() {
        let mut rt = JsRuntime::new(RuntimeOptions::default());
        let scope = &mut rt.handle_scope();

        // Parent realm Object identity (for distinctness check).
        let parent_obj_hash = {
            let g = scope.get_current_context().global(scope);
            let k = v8::String::new(scope, "Object").unwrap();
            let o = g.get(scope, k.into()).unwrap();
            v8::Local::<v8::Object>::try_from(o)
                .unwrap()
                .get_identity_hash()
        };

        let (ctx_g, _glob_g) =
            create_child_realm(scope).expect("child realm created");

        let ctx = v8::Local::new(scope, &ctx_g);
        let cs = &mut v8::ContextScope::new(scope, ctx);

        // Run a probe IN the child realm.
        let src = v8::String::new(
            cs,
            "JSON.stringify({\
               objTS: Function.prototype.toString.call(Object),\
               fnName: Function.name,\
               objName: Object.name,\
               arrTS: Array.prototype.slice.toString(),\
               typeofWin: typeof globalThis,\
               isProxyish: (function(){try{return String(globalThis).indexOf('Proxy')>=0}catch(e){return 'err'}})()\
             })",
        )
        .unwrap();
        let script = v8::Script::compile(cs, src, None).unwrap();
        let res = script.run(cs).unwrap();
        let json = res.to_rust_string_lossy(cs);

        // Child intrinsics must be GENUINE natives.
        assert!(
            json.contains("function Object() { [native code] }"),
            "child Object must be a real native, got: {json}"
        );
        assert!(
            json.contains("function slice() { [native code] }"),
            "child Array.prototype.slice must be native, got: {json}"
        );
        assert!(
            json.contains("\"objName\":\"Object\""),
            "child Object.name must be 'Object', got: {json}"
        );

        // Child global Object must be a DISTINCT object from parent's.
        let child_obj_hash = {
            let g = ctx.global(cs);
            let k = v8::String::new(cs, "Object").unwrap();
            let o = g.get(cs, k.into()).unwrap();
            v8::Local::<v8::Object>::try_from(o)
                .unwrap()
                .get_identity_hash()
        };
        assert_ne!(
            parent_obj_hash, child_obj_hash,
            "child realm Object must be realm-distinct from parent \
             (real per-frame realm, not parent-aliased)"
        );
    }
}
