use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
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
    #[allow(dead_code)]
    fn open(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;

        Ok(Self {
            file: Arc::new(Mutex::new(file)),
        })
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_keylog_path() -> std::path::PathBuf {
        let unique = format!(
            "browser-oxide-keylog-{}-{}.log",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );

        std::env::temp_dir().join(unique)
    }

    #[test]
    fn creates_file_and_writes_keylog_line() {
        let path = temporary_keylog_path();

        let writer = KeyLogWriter::open(&path).expect("open keylog file");
        assert!(path.is_file(), "keylog file was not created");

        writer.write_line(
            "CLIENT_HANDSHAKE_TRAFFIC_SECRET \
             000102030405060708090a0b0c0d0e0f \
             101112131415161718191a1b1c1d1e1f",
        );

        let contents = fs::read_to_string(&path).expect("read keylog file");
        assert_eq!(
            contents,
            "CLIENT_HANDSHAKE_TRAFFIC_SECRET \
             000102030405060708090a0b0c0d0e0f \
             101112131415161718191a1b1c1d1e1f\n"
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn rustls_keylog_callback_writes_nss_format() {
        let path = temporary_keylog_path();

        let writer = KeyLogWriter::open(&path).expect("open keylog file");

        rustls::KeyLog::log(
            &writer,
            "CLIENT_TRAFFIC_SECRET_0",
            &[0x00, 0x01, 0xab, 0xff],
            &[0x10, 0x20, 0xde, 0xad],
        );

        let contents = fs::read_to_string(&path).expect("read keylog file");
        assert_eq!(contents, "CLIENT_TRAFFIC_SECRET_0 0001abff 1020dead\n");

        let _ = fs::remove_file(path);
    }
}
