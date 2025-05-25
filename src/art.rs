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
}

// Helper function for serde skip_serializing_if
fn is_zero(value: &i32) -> bool {
    *value == 0
}

// Function to load a predefined pixel art
// In the future, this could load from a file (e.g., JSON)
pub fn load_default_pixel_art() -> PixelArt {
    PixelArt {
        name: "Smiley Face".to_string(),
        width: 4,  // Set appropriate width
        height: 3, // Set appropriate height
        pattern: vec![
            // Top row outline
            ArtPixel {
                x: 0,
                y: 0,
                color: 2,
            }, // Black outline
            ArtPixel {
                x: 1,
                y: 0,
                color: 2,
            },
            ArtPixel {
                x: 2,
                y: 0,
                color: 2,
            },
            // Middle row - eyes and outline
            ArtPixel {
                x: 0,
                y: 1,
                color: 2,
            }, // Left outline
            ArtPixel {
                x: 1,
                y: 1,
                color: 3,
            }, // Left eye (yellow)
            ArtPixel {
                x: 2,
                y: 1,
                color: 3,
            }, // Right eye (yellow)
            ArtPixel {
                x: 3,
                y: 1,
                color: 2,
            }, // Right outline
            // Bottom row - smile and outline
            ArtPixel {
                x: 0,
                y: 2,
                color: 2,
            }, // Left outline
            ArtPixel {
                x: 1,
                y: 2,
                color: 3,
            }, // Smile (yellow)
            ArtPixel {
                x: 2,
                y: 2,
                color: 2,
            }, // Right outline
        ],
        board_x: 10, // Default position on board
        board_y: 5,
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
