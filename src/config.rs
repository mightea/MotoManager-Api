use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub port: u16,
    pub rp_id: String,
    pub rp_name: String,
    pub origin: String,
    pub enable_registration: bool,
    pub app_version: String,
    pub data_dir: String,
    pub cache_dir: String,
    /// OpenAI-compatible LLM endpoint for invoice parsing (e.g. a local vLLM,
    /// "http://10.0.0.2:8542/v1"). Only ever called from this server — the
    /// LLM is never exposed to clients. None disables LLM structuring; the
    /// deterministic fallback parser then carries the feature alone.
    pub llm_base_url: Option<String>,
    pub llm_model: String,
    pub llm_api_key: String,
    /// Automatic database backups. `backup_enabled` gates the background
    /// scheduler only — the admin "Back up now" endpoint always works.
    /// `backup_interval_hours` is how often the scheduler runs; `backup_keep` is
    /// how many archives to retain (older ones are pruned after each run).
    pub backup_enabled: bool,
    pub backup_interval_hours: u64,
    pub backup_keep: usize,
    /// Frontend version to stamp into backups. The backend can't otherwise know
    /// it (separate image/deploy); set `FRONTEND_VERSION` alongside the frontend
    /// tag so scheduled backups record it. Manual backups from the webapp send
    /// their own version, which takes precedence. None = "unknown".
    pub frontend_version: Option<String>,
    /// `Host` header allowlist for the MCP endpoint (`MCP_ALLOWED_HOSTS`,
    /// comma-separated, entries may carry a port). Empty disables the check.
    /// The check guards against DNS rebinding of *local* servers; here every
    /// `/mcp` request is already rejected before reaching the MCP layer unless
    /// it carries a valid API token, so the default is off and the option is
    /// for deployments that want defence in depth.
    pub mcp_allowed_hosts: Vec<String>,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Config {
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite:./db.sqlite".to_string()),
            port: env::var("PORT")
                .unwrap_or_else(|_| "3001".to_string())
                .parse()
                .unwrap_or(3001),
            rp_id: env::var("RP_ID").unwrap_or_else(|_| "localhost".to_string()),
            rp_name: env::var("RP_NAME").unwrap_or_else(|_| "MotoManager".to_string()),
            origin: env::var("ORIGIN").unwrap_or_else(|_| "http://localhost:5174".to_string()),
            enable_registration: env::var("ENABLE_REGISTRATION")
                .unwrap_or_else(|_| "false".to_string())
                .to_lowercase()
                == "true",
            app_version: env::var("APP_VERSION")
                .unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string()),
            data_dir: env::var("DATA_DIR").unwrap_or_else(|_| "./data".to_string()),
            cache_dir: env::var("CACHE_DIR").unwrap_or_else(|_| "./cache".to_string()),
            llm_base_url: env::var("LLM_BASE_URL")
                .ok()
                .filter(|v| !v.trim().is_empty()),
            llm_model: env::var("LLM_MODEL")
                .unwrap_or_else(|_| "Qwen/Qwen2.5-1.5B-Instruct-AWQ".to_string()),
            llm_api_key: env::var("LLM_API_KEY").unwrap_or_else(|_| "local-vllm".to_string()),
            backup_enabled: env::var("BACKUP_ENABLED")
                .unwrap_or_else(|_| "true".to_string())
                .to_lowercase()
                != "false",
            backup_interval_hours: env::var("BACKUP_INTERVAL_HOURS")
                .ok()
                .and_then(|v| v.parse().ok())
                .filter(|&h| h > 0)
                .unwrap_or(24),
            backup_keep: env::var("BACKUP_KEEP")
                .ok()
                .and_then(|v| v.parse().ok())
                .filter(|&n| n > 0)
                .unwrap_or(14),
            frontend_version: env::var("FRONTEND_VERSION")
                .ok()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty()),
            mcp_allowed_hosts: env::var("MCP_ALLOWED_HOSTS")
                .ok()
                .map(|v| {
                    v.split(',')
                        .map(str::trim)
                        .filter(|h| !h.is_empty() && *h != "*")
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default(),
        })
    }

    /// Where backup archives (and the transient DB snapshot) are written. Kept
    /// under `data_dir` so it rides the same persistent volume as the DB.
    pub fn backup_dir(&self) -> std::path::PathBuf {
        std::path::Path::new(&self.data_dir).join("backups")
    }

    pub fn images_dir(&self) -> std::path::PathBuf {
        std::path::Path::new(&self.data_dir).join("images")
    }

    pub fn documents_dir(&self) -> std::path::PathBuf {
        std::path::Path::new(&self.data_dir).join("documents")
    }

    pub fn previews_dir(&self) -> std::path::PathBuf {
        std::path::Path::new(&self.cache_dir).join("previews")
    }

    pub fn resized_images_dir(&self) -> std::path::PathBuf {
        std::path::Path::new(&self.cache_dir).join("resized")
    }
}
