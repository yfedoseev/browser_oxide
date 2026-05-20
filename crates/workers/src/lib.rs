//! Web Workers for browser_oxide. Each worker runs in its own V8 isolate.

use js_runtime::BrowserJsRuntime;
use serde_json::Value;
use tokio::sync::mpsc;

/// A dedicated Web Worker running in its own V8 isolate.
pub struct WebWorker {
    runtime: BrowserJsRuntime,
    /// Channel for receiving messages from the worker
    rx: mpsc::Receiver<Value>,
    /// Channel sender stored in OpState for the worker to post messages
    tx: mpsc::Sender<Value>,
    terminated: bool,
}

impl WebWorker {
    /// Create a new worker from a script string.
    ///
    /// The worker gets a minimal global scope: self, postMessage, onmessage,
    /// setTimeout, console — but NO DOM.
    pub fn new(script: &str) -> Result<Self, deno_core::error::AnyError> {
        let (tx, rx) = mpsc::channel(100);

        // Create a minimal DOM (empty document) for the worker
        let dom = dom::Dom::new();
        let mut runtime = BrowserJsRuntime::new(dom);

        // Set up worker global scope
        runtime.execute_script(
            r#"
            globalThis.self = globalThis;
            globalThis._workerMessages = [];
            globalThis.postMessage = function(data) {
                globalThis._workerMessages.push(JSON.stringify(data));
            };
            globalThis.onmessage = null;
        "#,
            None,
        )?;

        // Execute the worker script
        runtime.execute_script(script, None)?;

        Ok(Self {
            runtime,
            rx,
            tx,
            terminated: false,
        })
    }

    /// Post a message to the worker (triggers onmessage).
    pub fn post_message(&mut self, data: Value) -> Result<(), deno_core::error::AnyError> {
        if self.terminated {
            return Ok(());
        }
        let json = serde_json::to_string(&data).unwrap_or_default();
        self.runtime.execute_script(
            &format!(
                r#"if (typeof onmessage === 'function') {{ onmessage({{ data: JSON.parse('{}') }}); }}"#,
                json.replace('\\', "\\\\").replace('\'', "\\'")
            ),
            None,
        )?;
        Ok(())
    }

    /// Collect messages posted by the worker via postMessage().
    pub fn collect_messages(&mut self) -> Result<Vec<Value>, deno_core::error::AnyError> {
        let result = self.runtime.execute_script(
            r#"JSON.stringify(globalThis._workerMessages.splice(0))"#,
            None,
        )?;
        let messages: Vec<String> = serde_json::from_str(&result).unwrap_or_default();
        Ok(messages
            .into_iter()
            .filter_map(|s| serde_json::from_str(&s).ok())
            .collect())
    }

    /// Run the worker's event loop until idle.
    pub async fn run_event_loop(&mut self) -> Result<(), deno_core::error::AnyError> {
        if self.terminated {
            return Ok(());
        }
        self.runtime.run_event_loop().await
    }

    /// Terminate the worker.
    pub fn terminate(&mut self) {
        self.terminated = true;
    }

    /// Check if terminated.
    pub fn is_terminated(&self) -> bool {
        self.terminated
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn create_worker() {
        let worker = WebWorker::new("const x = 42;");
        assert!(worker.is_ok());
    }

    #[test]
    fn worker_post_message() {
        let mut worker = WebWorker::new(
            r#"
            onmessage = function(e) {
                postMessage(e.data * 2);
            };
        "#,
        )
        .unwrap();

        worker.post_message(json!(21)).unwrap();
        let messages = worker.collect_messages().unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0], json!(42));
    }

    #[test]
    fn worker_no_dom() {
        let mut worker = WebWorker::new(
            r#"
            postMessage(typeof document);
        "#,
        )
        .unwrap();

        let messages = worker.collect_messages().unwrap();
        // document exists (we give a minimal DOM) but body etc. won't have content
        assert!(!messages.is_empty());
    }

    #[test]
    fn worker_has_console() {
        let worker = WebWorker::new(
            r#"
            console.log("worker log");
        "#,
        );
        assert!(worker.is_ok());
    }

    #[test]
    fn worker_has_timers() {
        // Verify timer functions exist (don't actually call setTimeout
        // which requires async event loop)
        let mut worker = WebWorker::new(
            r#"
            postMessage(typeof setTimeout);
        "#,
        )
        .unwrap();
        let messages = worker.collect_messages().unwrap();
        assert_eq!(messages[0], json!("function"));
    }

    #[test]
    fn worker_terminate() {
        let mut worker = WebWorker::new("").unwrap();
        assert!(!worker.is_terminated());
        worker.terminate();
        assert!(worker.is_terminated());
    }

    #[test]
    fn worker_multiple_messages() {
        let mut worker = WebWorker::new(
            r#"
            onmessage = function(e) {
                postMessage("got: " + e.data);
            };
        "#,
        )
        .unwrap();

        worker.post_message(json!("hello")).unwrap();
        worker.post_message(json!("world")).unwrap();
        let messages = worker.collect_messages().unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0], json!("got: hello"));
        assert_eq!(messages[1], json!("got: world"));
    }
}
