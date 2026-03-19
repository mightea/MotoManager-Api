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
            origin: env::var("ORIGIN")
                .unwrap_or_else(|_| "http://localhost:5173".to_string()),
            enable_registration: env::var("ENABLE_REGISTRATION")
                .unwrap_or_else(|_| "false".to_string())
                .to_lowercase()
                == "true",
            app_version: env::var("APP_VERSION")
                .unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string()),
            data_dir: env::var("DATA_DIR").unwrap_or_else(|_| "./data".to_string()),
            cache_dir: env::var("CACHE_DIR").unwrap_or_else(|_| "./cache".to_string()),
        })
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

