use std::env;
use std::path::PathBuf;

const DEFAULT_MEDIA_DRIVER_TIMEOUT_MS: u16 = 10_000;
const DEFAULT_RESOURCE_LINGER_MS: u16 = 5_000;

pub struct Context {
    aeron_dir: PathBuf,
    media_driver_timeout_ms: i32,
    resource_linger_timeout_ms: i32,
    use_conductor_agent_invoker: bool,
    pre_touch_mapped_memory: bool,
}

impl Context {
    pub fn get_user_name() -> String {
        env::var("USER")
            .or_else(|_| env::var("USERNAME"))
            .unwrap_or("default".to_string())
    }

    pub fn default_aeron_path() -> PathBuf {
        let base_path = if cfg!(target_os = "linux") {
            PathBuf::from("/dev/shm")
        } else {
            // Uses TMPDIR on Unix-like, and GetTempPath on Windows
            env::temp_dir()
        };

        base_path.join(format!("aeron-{}", Context::get_user_name()))
    }
}

impl Default for Context {
    fn default() -> Self {
        Context {
            aeron_dir: Context::default_aeron_path(),
            media_driver_timeout_ms: DEFAULT_MEDIA_DRIVER_TIMEOUT_MS.into(),
            resource_linger_timeout_ms: DEFAULT_RESOURCE_LINGER_MS.into(),
            use_conductor_agent_invoker: false,
            pre_touch_mapped_memory: false,
        }
    }
}
