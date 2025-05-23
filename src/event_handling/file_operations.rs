use crate::app_state::App;
use crate::art::PixelArt;
use std::fs::File;
use std::io::Write;
use std::path::Path;

impl App {
    /// Save current art in editor to file
    pub async fn save_current_art_to_file(&mut self, filename: String) {
        if let Some(art) = &self.current_editing_art {
            // Use the art's existing name, and preserve any positioning
            let art_with_name = PixelArt {
                name: art.name.clone(),
                pixels: art.pixels.clone(),
                board_x: art.board_x, // Preserve position for queue automation
                board_y: art.board_y,
            };
            match serde_json::to_string_pretty(&art_with_name) {
                Ok(json_data) => {
                    let dir_path = Path::new("pixel_arts");
                    if !dir_path.exists() {
                        if let Err(e) = std::fs::create_dir_all(dir_path) {
                            self.status_message =
                                format!("Error creating directory pixel_arts: {}", e);
                            return;
                        }
                    }
                    let file_path = dir_path.join(if filename.ends_with(".json") {
                        filename
                    } else {
                        format!("{}.json", filename)
                    });
                    match File::create(&file_path) {
                        Ok(mut file) => {
                            if let Err(e) = file.write_all(json_data.as_bytes()) {
                                self.status_message =
                                    format!("Error writing to file {}: {}", file_path.display(), e);
                            } else {
                                self.status_message = format!(
                                    "Art '{}' saved to {}",
                                    art_with_name.name,
                                    file_path.display()
                                );
                            }
                        }
                        Err(e) => {
                            self.status_message =
                                format!("Error creating file {}: {}", file_path.display(), e);
                        }
                    }
                }
                Err(e) => {
                    self.status_message = format!("Error serializing art to JSON: {}", e);
                }
            }
        } else {
            self.status_message = "No current art to save.".to_string();
        }
    }

    /// Save current tokens and base URL to persistent storage
    pub fn save_tokens(&mut self) {
        let token_data = crate::token_storage::TokenData {
            access_token: self.api_client.get_access_token_clone(),
            refresh_token: self.api_client.get_refresh_token_clone(),
            base_url: Some(self.api_client.get_base_url()),
        };

        if let Err(e) = self.token_storage.save(&token_data) {
            eprintln!("Warning: Could not save tokens: {}", e);
        }
    }

    /// Clear saved tokens from persistent storage
    pub fn clear_saved_tokens(&mut self) {
        if let Err(e) = self.token_storage.clear() {
            eprintln!("Warning: Could not clear saved tokens: {}", e);
        }
    }
}
