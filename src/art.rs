use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

// Pixel Art Structures
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ArtPixel {
    pub x: i32,     // Relative X offset from top-left of the art
    pub y: i32,     // Relative Y offset
    pub color: i32, // Changed from color_id to color to match dofus2.json format
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct PixelArt {
    pub name: String,
    pub width: i32,             // Added width field
    pub height: i32,            // Added height field
    pub pattern: Vec<ArtPixel>, // Changed from pixels to pattern

    // Optional fields for runtime positioning (not saved to file by default)
    #[serde(default, skip_serializing_if = "is_zero")]
    pub board_x: i32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub board_y: i32,

    // New metadata fields for sharing
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>, // ISO 8601 timestamp
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}

// Shareable pixel art format with coordinates
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ShareablePixelArt {
    pub art: PixelArt,
    pub board_x: i32,
    pub board_y: i32,
    pub share_message: Option<String>,
    pub shared_by: Option<String>,
    pub shared_at: String, // ISO 8601 timestamp
}

// Helper function for serde skip_serializing_if
fn is_zero(value: &i32) -> bool {
    *value == 0
}

// Function to load a predefined pixel art
// Swiss flag as the default pixel art
pub fn load_default_pixel_art() -> PixelArt {
    PixelArt {
        name: "Swiss".to_string(),
        width: 5, // 5x5 Swiss flag
        height: 5,
        pattern: vec![
            // White cross pixels (color 1)
            ArtPixel {
                x: 2,
                y: 3,
                color: 1,
            },
            ArtPixel {
                x: 2,
                y: 2,
                color: 1,
            },
            ArtPixel {
                x: 2,
                y: 1,
                color: 1,
            },
            ArtPixel {
                x: 1,
                y: 2,
                color: 1,
            },
            ArtPixel {
                x: 3,
                y: 2,
                color: 1,
            },
            // Red background pixels (color 18)
            ArtPixel {
                x: 1,
                y: 4,
                color: 18,
            },
            ArtPixel {
                x: 2,
                y: 4,
                color: 18,
            },
            ArtPixel {
                x: 3,
                y: 4,
                color: 18,
            },
            ArtPixel {
                x: 1,
                y: 3,
                color: 18,
            },
            ArtPixel {
                x: 0,
                y: 3,
                color: 18,
            },
            ArtPixel {
                x: 0,
                y: 2,
                color: 18,
            },
            ArtPixel {
                x: 0,
                y: 1,
                color: 18,
            },
            ArtPixel {
                x: 0,
                y: 0,
                color: 18,
            },
            ArtPixel {
                x: 1,
                y: 0,
                color: 18,
            },
            ArtPixel {
                x: 2,
                y: 0,
                color: 18,
            },
            ArtPixel {
                x: 3,
                y: 0,
                color: 18,
            },
            ArtPixel {
                x: 4,
                y: 0,
                color: 18,
            },
            ArtPixel {
                x: 4,
                y: 1,
                color: 18,
            },
            ArtPixel {
                x: 4,
                y: 2,
                color: 18,
            },
            ArtPixel {
                x: 4,
                y: 3,
                color: 18,
            },
            ArtPixel {
                x: 4,
                y: 4,
                color: 18,
            },
            ArtPixel {
                x: 0,
                y: 4,
                color: 18,
            },
            ArtPixel {
                x: 1,
                y: 1,
                color: 18,
            },
            ArtPixel {
                x: 3,
                y: 1,
                color: 18,
            },
            ArtPixel {
                x: 3,
                y: 3,
                color: 18,
            },
        ],
        board_x: 10, // Default position on board
        board_y: 5,
        description: Some("Swiss flag with white cross on red background".to_string()),
        author: Some("ftplace-TUI".to_string()),
        created_at: Some(chrono::Utc::now().to_rfc3339()),
        tags: Some(vec![
            "default".to_string(),
            "swiss".to_string(),
            "flag".to_string(),
        ]),
    }
}

/// Load a pixel art from a JSON file
pub fn load_pixel_art_from_file(file_path: &Path) -> Result<PixelArt, Box<dyn std::error::Error>> {
    let file_content = fs::read_to_string(file_path)?;
    let pixel_art: PixelArt = serde_json::from_str(&file_content)?;

    // Preserve the saved board position instead of resetting
    // This allows for template positioning and queue automation

    Ok(pixel_art)
}

/// Load a shareable pixel art from a JSON file
pub fn load_shareable_pixel_art_from_file(
    file_path: &Path,
) -> Result<ShareablePixelArt, Box<dyn std::error::Error>> {
    let file_content = fs::read_to_string(file_path)?;
    let shareable_art: ShareablePixelArt = serde_json::from_str(&file_content)?;
    Ok(shareable_art)
}

/// Save a pixel art as a shareable format with coordinates
pub fn save_shareable_pixel_art(
    art: &PixelArt,
    board_x: i32,
    board_y: i32,
    share_message: Option<String>,
    shared_by: Option<String>,
    file_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let shareable = ShareablePixelArt {
        art: art.clone(),
        board_x,
        board_y,
        share_message,
        shared_by,
        shared_at: chrono::Utc::now().to_rfc3339(),
    };

    // Create directory if it doesn't exist
    if let Some(parent) = file_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let json_data = serde_json::to_string_pretty(&shareable)?;
    std::fs::write(file_path, json_data)?;
    Ok(())
}

/// Generate a shareable coordinate string for easy copy-paste
pub fn generate_share_string(art: &PixelArt, board_x: i32, board_y: i32) -> String {
    format!(
        "ftplace-share: {} at ({}, {}) - {} pixels",
        art.name,
        board_x,
        board_y,
        art.pattern.len()
    )
}

/// Parse a share string to extract coordinates
pub fn parse_share_string(share_string: &str) -> Option<(String, i32, i32)> {
    if !share_string.starts_with("ftplace-share:") {
        return None;
    }

    // Extract name and coordinates from "ftplace-share: NAME at (X, Y) - N pixels"
    let parts: Vec<&str> = share_string.split(" at (").collect();
    if parts.len() != 2 {
        return None;
    }

    let name = parts[0].trim_start_matches("ftplace-share:").trim();

    let coord_part = parts[1].split(')').next()?;
    let coords: Vec<&str> = coord_part.split(", ").collect();
    if coords.len() != 2 {
        return None;
    }

    let x = coords[0].parse::<i32>().ok()?;
    let y = coords[1].parse::<i32>().ok()?;

    Some((name.to_string(), x, y))
}

/// Get all available pixel arts (saved files + default)
pub fn get_available_pixel_arts() -> Vec<PixelArt> {
    let mut arts = Vec::new();

    // Add default pixel art first
    arts.push(load_default_pixel_art());

    // Load saved pixel arts from pixel_arts directory
    let pixel_arts_dir = Path::new("pixel_arts");
    if pixel_arts_dir.exists() && pixel_arts_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(pixel_arts_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("json") {
                    if let Ok(pixel_art) = load_pixel_art_from_file(&path) {
                        arts.push(pixel_art);
                    }
                }
            }
        }
    }

    arts
}

/// Get all available shareable pixel arts from shares directory
pub fn get_available_shareable_arts() -> Vec<ShareablePixelArt> {
    let mut arts = Vec::new();

    let shares_dir = Path::new("shares");
    if shares_dir.exists() && shares_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(shares_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("json") {
                    if let Ok(shareable_art) = load_shareable_pixel_art_from_file(&path) {
                        arts.push(shareable_art);
                    }
                }
            }
        }
    }

    arts
}

/// Get dimensions of a pixel art (width, height)
pub fn get_art_dimensions(art: &PixelArt) -> (i32, i32) {
    if art.pattern.is_empty() {
        return (0, 0);
    }

    let min_x = art.pattern.iter().map(|p| p.x).min().unwrap_or(0);
    let max_x = art.pattern.iter().map(|p| p.x).max().unwrap_or(0);
    let min_y = art.pattern.iter().map(|p| p.y).min().unwrap_or(0);
    let max_y = art.pattern.iter().map(|p| p.y).max().unwrap_or(0);

    (max_x - min_x + 1, max_y - min_y + 1)
}
