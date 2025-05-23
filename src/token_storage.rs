use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TokenData {
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub base_url: Option<String>,
}

impl Default for TokenData {
    fn default() -> Self {
        Self {
            access_token: None,
            refresh_token: None,
            base_url: None,
        }
    }
}

#[derive(Debug)]
pub struct TokenStorage {
    file_path: PathBuf,
}

impl TokenStorage {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let home_dir = dirs::home_dir().ok_or("Could not find home directory")?;
        let file_path = home_dir.join(".ftplace_tokens.json");

        Ok(Self { file_path })
    }

    pub fn load(&self) -> TokenData {
        match self.try_load() {
            Ok(data) => {
                // eprintln!("Loaded saved tokens from {}", self.file_path.display());
                data
            }
            Err(e) => {
                eprintln!("Could not load saved tokens: {}. Starting fresh.", e);
                TokenData::default()
            }
        }
    }

    fn try_load(&self) -> Result<TokenData, Box<dyn std::error::Error>> {
        if !self.file_path.exists() {
            return Ok(TokenData::default());
        }

        let content = fs::read_to_string(&self.file_path)?;
        let data: TokenData = serde_json::from_str(&content)?;
        Ok(data)
    }

    pub fn save(&self, data: &TokenData) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string_pretty(data)?;
        fs::write(&self.file_path, json)?;

        // Set file permissions to be readable/writable only by owner (600)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&self.file_path)?.permissions();
            perms.set_mode(0o600);
            fs::set_permissions(&self.file_path, perms)?;
        }

        Ok(())
    }

    pub fn clear(&self) -> Result<(), Box<dyn std::error::Error>> {
        if self.file_path.exists() {
            fs::remove_file(&self.file_path)?;
        }
        Ok(())
    }

    pub fn get_file_path(&self) -> &PathBuf {
        &self.file_path
    }
}
