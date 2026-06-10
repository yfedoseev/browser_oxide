//! Python bindings for browser_oxide (PyO3).
//!
//! Idiomatic Python surface over [`browser_oxide_host`]: `Browser`, `Page`,
//! `Profile`. The engine is `!Send` (per-thread V8); `browser_oxide_host` runs
//! it on a dedicated thread, so the `Browser`/`Page` objects are ordinary
//! GIL-friendly Python objects and the GIL is released during navigation.

use std::sync::Arc;

use browser_oxide::host::{stealth::presets, EngineHandle, PageSnapshot, StealthProfile};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

fn to_pyerr<E: std::fmt::Display>(e: E) -> PyErr {
    PyRuntimeError::new_err(e.to_string())
}

/// A browser identity (TLS + headers + navigator + GPU + fingerprint seeds).
#[pyclass(module = "browser_oxide._native", from_py_object)]
#[derive(Clone)]
struct Profile {
    inner: StealthProfile,
}

#[pymethods]
impl Profile {
    /// Chrome 148 on macOS (the default).
    #[staticmethod]
    fn chrome() -> Self {
        Self {
            inner: presets::chrome_148_macos(),
        }
    }
    /// Firefox 135 on macOS (real NSS ClientHello).
    #[staticmethod]
    fn firefox() -> Self {
        Self {
            inner: presets::firefox_135_macos(),
        }
    }
    /// Safari 18 on iPhone 15 Pro.
    #[staticmethod]
    fn iphone() -> Self {
        Self {
            inner: presets::iphone_15_pro_safari_18(),
        }
    }
    /// Chrome 148 on Pixel 9 Pro (Android).
    #[staticmethod]
    fn pixel() -> Self {
        Self {
            inner: presets::pixel_9_pro_chrome_148(),
        }
    }
    /// Load a custom profile from a YAML or JSON file.
    #[staticmethod]
    fn from_file(path: &str) -> PyResult<Self> {
        StealthProfile::load_from_file(path)
            .map(|inner| Self { inner })
            .map_err(to_pyerr)
    }

    fn __repr__(&self) -> String {
        format!("Profile(user_agent={:?})", self.inner.user_agent)
    }
}

/// A stealth headless browser. Spawns the engine thread on construction; the
/// thread is shut down when the object (and any pages from it) are dropped.
#[pyclass(module = "browser_oxide._native")]
struct Browser {
    engine: Arc<EngineHandle>,
    profile: StealthProfile,
}

#[pymethods]
impl Browser {
    #[new]
    #[pyo3(signature = (profile=None))]
    fn new(profile: Option<Profile>) -> Self {
        Browser {
            engine: Arc::new(EngineHandle::spawn()),
            profile: profile
                .map(|p| p.inner)
                .unwrap_or_else(presets::chrome_148_macos),
        }
    }

    /// Navigate to `url` and return a `Page`. Releases the GIL while the engine
    /// thread does the work.
    #[pyo3(signature = (url, max_iterations=5))]
    fn navigate(&self, py: Python<'_>, url: &str, max_iterations: u8) -> PyResult<Page> {
        let engine = Arc::clone(&self.engine);
        let profile = self.profile.clone();
        let url = url.to_string();
        let snap = py
            .detach(move || engine.navigate(&url, profile, max_iterations))
            .map_err(to_pyerr)?;
        Ok(Page {
            engine: Arc::clone(&self.engine),
            snap,
        })
    }

    /// Evaluate JS against the most recently navigated page.
    fn evaluate(&self, py: Python<'_>, js: &str) -> PyResult<String> {
        let engine = Arc::clone(&self.engine);
        let js = js.to_string();
        py.detach(move || engine.evaluate(&js)).map_err(to_pyerr)
    }

    /// `querySelector(selector).textContent` against the current page.
    fn query_text(&self, py: Python<'_>, selector: &str) -> PyResult<Option<String>> {
        let engine = Arc::clone(&self.engine);
        let selector = selector.to_string();
        py.detach(move || engine.query_text(&selector))
            .map_err(to_pyerr)
    }

    /// Idempotent; the engine thread is also stopped on drop.
    fn close(&self) {}

    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    #[pyo3(signature = (_exc_type=None, _exc_value=None, _traceback=None))]
    fn __exit__(
        &self,
        _exc_type: Option<Py<pyo3::PyAny>>,
        _exc_value: Option<Py<pyo3::PyAny>>,
        _traceback: Option<Py<pyo3::PyAny>>,
    ) -> bool {
        false
    }
}

/// A rendered page. Accessors are properties; `evaluate`/`query_text` run
/// against the engine's current page.
#[pyclass(module = "browser_oxide._native")]
struct Page {
    engine: Arc<EngineHandle>,
    snap: PageSnapshot,
}

#[pymethods]
impl Page {
    #[getter]
    fn url(&self) -> &str {
        &self.snap.url
    }
    #[getter]
    fn title(&self) -> &str {
        &self.snap.title
    }
    #[getter]
    fn html(&self) -> &str {
        &self.snap.html
    }
    #[getter]
    fn text(&self) -> &str {
        &self.snap.text
    }
    /// One of: pass / thin-shell / render-incomplete / edge-block / sensor-fail
    /// / challenge-incomplete (compares equal to the Python `Verdict` enum).
    #[getter]
    fn verdict(&self) -> &str {
        &self.snap.verdict
    }
    #[getter]
    fn is_challenge(&self) -> bool {
        self.snap.is_challenge
    }

    fn evaluate(&self, py: Python<'_>, js: &str) -> PyResult<String> {
        let engine = Arc::clone(&self.engine);
        let js = js.to_string();
        py.detach(move || engine.evaluate(&js)).map_err(to_pyerr)
    }

    fn query_text(&self, py: Python<'_>, selector: &str) -> PyResult<Option<String>> {
        let engine = Arc::clone(&self.engine);
        let selector = selector.to_string();
        py.detach(move || engine.query_text(&selector))
            .map_err(to_pyerr)
    }

    fn __repr__(&self) -> String {
        format!(
            "Page(url={:?}, verdict={:?}, bytes={})",
            self.snap.url,
            self.snap.verdict,
            self.snap.html.len()
        )
    }
}

#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Browser>()?;
    m.add_class::<Page>()?;
    m.add_class::<Profile>()?;
    Ok(())
}
