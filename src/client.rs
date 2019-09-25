//! Client library for Aeron. This encapsulates the logic needed to communicate
//! with the media driver, but does not manage the media driver itself.
use std::env;
use std::path::PathBuf;

/// Context used to initialize the Aeron client
pub struct ClientContext {
    aeron_dir: PathBuf,
}

impl ClientContext {
    fn get_user_name() -> String {
        env::var("USER")
            .or_else(|_| env::var("USERNAME"))
            .unwrap_or_else(|_| "default".to_string())
    }

    /// Get the default folder used by the Media Driver to interact with clients
    pub fn default_aeron_path() -> PathBuf {
        let base_path = if cfg!(target_os = "linux") {
            PathBuf::from("/dev/shm")
        } else {
            // Uses TMPDIR on Unix-like and GetTempPath on Windows
            env::temp_dir()
        };

        base_path.join(format!("aeron-{}", ClientContext::get_user_name()))
    }
}

impl Default for ClientContext {
    fn default() -> Self {
        ClientContext {
            aeron_dir: ClientContext::default_aeron_path(),
        }
    }
}
