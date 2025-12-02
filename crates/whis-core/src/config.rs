use anyhow::{Context, Result};
use std::env;

pub struct ApiConfig {
    pub openai_api_key: String,
}

impl ApiConfig {
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok(); // Load .env file if it exists

        let openai_api_key = env::var("OPENAI_API_KEY")
            .context("OPENAI_API_KEY not found. Please set it in .env file or environment")?;

        Ok(ApiConfig { openai_api_key })
    }
}
