use serde::{Deserialize, Serialize};

// Pixel Art Structures
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ArtPixel {
    pub x: i32, // Relative X offset from top-left of the art
    pub y: i32, // Relative Y offset
    pub color_id: i32,
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct PixelArt {
    pub name: String,
    pub pixels: Vec<ArtPixel>,
    // position on the main board where the top-left of the art is placed
    pub board_x: i32,
    pub board_y: i32,
}

// Function to load a predefined pixel art
// In the future, this could load from a file (e.g., JSON)
pub fn load_default_pixel_art() -> PixelArt {
    PixelArt {
        name: "Smiley Face".to_string(),
        board_x: 10, // Default position on board
        board_y: 5,
        pixels: vec![
            ArtPixel {
                x: 0,
                y: 0,
                color_id: 2,
            }, // Example: Black for outline (assuming color_id 2 is black)
            ArtPixel {
                x: 1,
                y: 0,
                color_id: 2,
            },
            ArtPixel {
                x: 2,
                y: 0,
                color_id: 2,
            },
            ArtPixel {
                x: 0,
                y: 1,
                color_id: 3,
            },
            ArtPixel {
                x: 1,
                y: 1,
                color_id: 2,
            },
            ArtPixel {
                x: 2,
                y: 1,
                color_id: 3,
            },
            ArtPixel {
                x: 0,
                y: 2,
                color_id: 2,
            },
            ArtPixel {
                x: 1,
                y: 2,
                color_id: 3,
            },
            // Eyes (example: yellow, assuming color_id 3)
            ArtPixel {
                x: 0,
                y: 1,
                color_id: 3,
            },
            ArtPixel {
                x: 2,
                y: 1,
                color_id: 3,
            },
            // Smile (example: yellow)
            ArtPixel {
                x: 1,
                y: 2,
                color_id: 3,
            },
        ],
    }
}
