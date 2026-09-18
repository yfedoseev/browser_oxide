use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Clone, Debug)]
pub(crate) struct KeyLogWriter {
    file: Arc<Mutex<File>>,
}

static KEYLOG_WRITER: OnceLock<Option<KeyLogWriter>> = OnceLock::new();

pub(crate) fn keylog_writer() -> Option<KeyLogWriter> {
    KEYLOG_WRITER
        .get_or_init(|| {
            let path = std::env::var_os("SSLKEYLOGFILE")?;
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .ok()?;

            Some(KeyLogWriter {
                file: Arc::new(Mutex::new(file)),
            })
        })
        .clone()
}

impl KeyLogWriter {
    pub(crate) fn write_line(&self, line: &str) {
        if let Ok(mut file) = self.file.lock() {
            let _ = writeln!(file, "{line}");
            let _ = file.flush();
        }
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

impl rustls::KeyLog for KeyLogWriter {
    fn log(&self, label: &str, client_random: &[u8], secret: &[u8]) {
        let mut line = String::new();
        line.push_str(label);
        line.push(' ');
        line.push_str(&hex_encode(client_random));
        line.push(' ');
        line.push_str(&hex_encode(secret));
        self.write_line(&line);
    }

    fn will_log(&self, _label: &str) -> bool {
        true
    }
}
