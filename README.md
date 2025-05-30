# ftplace-TUI

A Terminal User Interface (TUI) application for automating pixel placement on ftplace websites (r/place clones). Built with Rust and Ratatui for a responsive, feature-rich terminal experience.

## 🎯 Target Platform

**Primary Target:** https://ftplace.42lausanne.ch/

## ✨ Features

### 🎨 Pixel Art Management

- **Create pixel art** directly in the TUI with a built-in editor
- **Load existing pixel art** from JSON files
- **Save/export** pixel art creations
- **Position art** interactively on the board with arrow keys or mouse
- **Preview placement** with real-time overlay visualization

### 🖼️ Live Board Visualization

- **Real-time board display** fetched from the ftplace API
- **Viewport navigation** with arrow keys and mouse scrolling
- **Half-block rendering** for high-resolution pixel display in terminal
- **Auto-refresh** every 10 seconds to stay synchronized
- **Color-accurate** representation using the server's color palette

### 🤖 Automated Pixel Placement

- **Queue-based system** for managing multiple pixel art placements
- **Priority management** (1=high, 5=low) for queue items
- **Smart cooldown handling** respecting API rate limits
- **Progress tracking** with visual feedback
- **Retry logic** for failed placements
- **Background processing** with real-time status updates

### 🔐 Authentication & Session Management

- **JWT token support** (access + refresh tokens)
- **Automatic token refresh** on 426 responses
- **Persistent token storage** between sessions
- **Multiple API endpoint** support with easy switching

### 📊 User Interface & Feedback

- **Multi-panel layout** with board, status, and controls
- **Real-time status updates** with emoji indicators
- **Timer display** showing pixel cooldowns and availability
- **Status log history** with timestamps (UTC+2)
- **Help system** with comprehensive command reference
- **Profile viewer** showing user stats and timers

### 🎯 Advanced Features

- **Mouse support** for positioning and placement
- **Share system** for coordinating with other users
- **Queue management** with pause/resume functionality
- **Smart pixel detection** (skips already-correct pixels)
- **Background color filtering** (ignores transparent/empty colors)
- **Persistent data** (queue, status messages, tokens)

## 🚀 Quick Start

### Prerequisites

- Rust (latest stable version)
- Terminal with Unicode and color support

### Installation

```bash
git clone <repository-url>
cd ftplace-TUI
make build
```

### First Run

```bash
make run
```

The application will guide you through initial setup:

1. **Select API endpoint** from predefined options or enter custom URL
2. **Enter access token** (JWT from browser cookies)
3. **Enter refresh token** (optional, for automatic token renewal)
4. **Board loads automatically** once configured

## 🎮 Controls & Navigation

### Main Interface

| Key | Action                    |
| --- | ------------------------- |
| `q` | Quit application          |
| `?` | Show help popup           |
| `h` | Show status log history   |
| `i` | Show user profile         |
| `r` | Refresh board data        |
| `p` | Fetch user profile/timers |
| `b` | Change API base URL       |
| `c` | Change access token       |

### Board Navigation

| Key            | Action                               |
| -------------- | ------------------------------------ |
| `↑↓←→`         | Scroll viewport (when no art loaded) |
| `Mouse Scroll` | Navigate board                       |
| `Left Click`   | Position loaded art                  |

### Art Management

| Key     | Action                                   |
| ------- | ---------------------------------------- |
| `l`     | Load/select pixel art                    |
| `e`     | Create new pixel art                     |
| `↑↓←→`  | Position loaded art (when art is loaded) |
| `Enter` | Load selected art for positioning        |
| `d`     | Delete selected art (with confirmation)  |
| `Esc`   | Cancel art selection                     |

### Queue Management

| Key | Action                                      |
| --- | ------------------------------------------- |
| `w` | Open work queue management                  |
| `s` | Toggle pause/resume for selected queue item |

### Art Editor

| Key         | Action                             |
| ----------- | ---------------------------------- |
| `↑↓←→`      | Move cursor                        |
| `Space`     | Draw pixel with selected color     |
| `Tab`       | Next color in palette              |
| `Shift+Tab` | Previous color in palette          |
| `s`         | Save current art                   |
| `Esc`       | Exit editor (unsaved changes lost) |

### Sharing System

| Key | Action                            |
| --- | --------------------------------- |
| `x` | Share loaded art with coordinates |
| `v` | View/import shared arts           |
| `z` | Enter share string manually       |

## 🔄 Application Flows

### 1. Initial Setup Flow

```
Start → Select API URL → Enter Access Token → Enter Refresh Token → Load Board → Ready
```

### 2. Art Creation Flow

```
Press 'e' → Enter Art Name → Art Editor → Draw Pixels → Save ('s') → Exit (Esc)
```

### 3. Art Placement Flow

```
Press 'l' → Select Art → Position with Arrows/Mouse → Press Enter → Queue Processing
```

### 4. Queue Management Flow

```
Press 'w' → View Queue → Reorder (u/j) → Set Priority (1-5) → Start (Enter)
```

### 5. Sharing Flow

```
Load Art → Position → Press 'x' → Enter Message → Generate Share String
```

## 📁 File Structure

```
ftplace-TUI/
├── src/
│   ├── main.rs              # Application entry point
│   ├── app_state.rs         # Core application state
│   ├── api_client.rs        # ftplace API integration
│   ├── art.rs              # Pixel art data structures
│   ├── token_storage.rs     # Persistent token management
│   ├── ui/                  # User interface modules
│   │   ├── render.rs        # Main rendering logic
│   │   ├── helpers.rs       # UI utility functions
│   │   ├── popups.rs        # Help, profile, status popups
│   │   ├── art_editor.rs    # Pixel art editor UI
│   │   └── art_management.rs # Art selection/queue UI
│   ├── event_handling/      # Input and event processing
│   │   ├── input_handling.rs # Keyboard/mouse input
│   │   ├── helpers.rs       # Event processing utilities
│   │   ├── board_management.rs # Board fetching/updates
│   │   ├── profile_management.rs # User profile handling
│   │   ├── art_placement.rs # Individual art placement
│   │   └── queue_management.rs # Queue processing
│   └── background_tasks/    # Async background operations
│       ├── board_fetcher.rs # Background board updates
│       ├── art_placer.rs    # Background pixel placement
│       └── queue_processor.rs # Background queue processing
├── logs/                    # Application logs
│   └── status_messages.json # Persistent status history
├── queue/                   # Queue data (planned)
│   └── queue.json          # Persistent queue state
├── patterns/               # Saved pixel art files
│   └── *.json             # Individual art files
└── README.md              # This file
```

## 🎨 Pixel Art Format

Pixel arts are stored as JSON files with the following structure:

```json
{
	"name": "Art Name",
	"width": 10,
	"height": 10,
	"pattern": [
		{ "x": 0, "y": 0, "color": 1 },
		{ "x": 1, "y": 0, "color": 2 }
	],
	"board_x": 100,
	"board_y": 100,
	"description": "Optional description",
	"author": "Optional author",
	"created_at": "2024-01-15T14:30:25Z",
	"tags": ["tag1", "tag2"]
}
```

## 🔧 Configuration

### API Endpoints

The application supports multiple predefined endpoints:

- `https://ftplace.42lausanne.ch` (primary target)
- `http://localhost:7979` (local development via `make run-local`)
- Custom URLs (entered manually)

### Token Management

The TUI implements robust token management to handle long-running operations:

### Token Refresh Mechanism

- The application automatically refreshes expired JWT tokens during API calls
- When the backend returns a 426 status code, new tokens are extracted from Set-Cookie headers
- **Automatic Persistence**: Refreshed tokens are automatically saved to `~/.ftplace_tokens.json`
- This ensures queue processing can continue overnight without interruption

### Background Task Token Handling

All background tasks (queue processing, board fetching, validation, etc.) now include token refresh callbacks that:

- Detect when tokens are refreshed during API operations
- Automatically save the new tokens to persistent storage
- Prevent token expiration from stopping long-running queue processing

### Token Storage

- Tokens are stored in `~/.ftplace_tokens.json` in your home directory
- File permissions are set to 600 (owner read/write only) for security
- Both access and refresh tokens are persisted along with the base URL

### Queue Persistence

The art queue is automatically saved to `queue.json` and restored between sessions, maintaining:

- Queue order and priorities
- Placement progress
- Pause states

## 🎯 Status Indicators

### Timer Status

- 🟢 **Green**: Pixels available for placement
- 🔴 **Red**: All pixels on cooldown
- 🟡 **Yellow**: No timer data available
- ⚪ **White**: No user info loaded

### API Call Status

- ✅ **Success**: 200-299 status codes
- ❌ **Client Error**: 400-499 status codes
- 💥 **Server Error**: 500-599 status codes
- 🔄 **Token Refresh**: 426 status code
- ⏳ **Pending**: Request in progress

### Queue Status

- 🔄 **Starting**: Queue processing initiated
- 📋 **Progress**: Pixels being placed
- ✅ **Completed**: Item finished successfully
- ❌ **Failed**: Item failed to complete
- ⏭️ **Skipped**: Item skipped (no changes needed)
- 🎉 **Complete**: Entire queue finished
- 🛑 **Cancelled**: Processing cancelled by user
- ⏸️ **Paused**: Processing paused
- ▶️ **Resumed**: Processing resumed

## 🔍 Troubleshooting

### Common Issues

**"Unauthorized access" errors:**

- Check that your access token is valid
- Try refreshing the page in browser and copying new tokens
- Ensure the API endpoint is correct

**Board not loading:**

- Verify internet connection
- Check API endpoint accessibility
- Try refreshing with 'r' key

**Pixel placement failures:**

- Check cooldown timers with 'p' key
- Verify you have pixels available in buffer
- Ensure target coordinates are within board bounds

**Performance issues:**

- Large pixel arts may take time to process
- Use queue system for multiple arts
- Monitor status log for detailed progress

### Debug Information

- Press 'h' to view detailed status log
- Press 'i' to check user profile and timers
- Monitor the status area for real-time feedback

## 🛠️ Development

### Building

```bash
# Build release version (default)
make build

# Build debug version
make build-debug

# Clean build artifacts
make clean

# Check code without building
make check
```

### Running

```bash
# Run debug version with token prompt
make run

# Run against local server (localhost:7979)
make run-local

# Or set environment variables and run
FTPLACE_ACCESS_TOKEN="your_token" FTPLACE_REFRESH_TOKEN="your_refresh" make run
```

### Code Formatting

```bash
make fmt
```

### Linting

```bash
make clippy
```

### Available Make Targets

```bash
# Show all available commands
make help
```

## 📝 License

[Add your license information here]

## 🤝 Contributing

[Add contribution guidelines here]

## 📞 Support

[Add support/contact information here]

---

**Note**: This application is designed for educational and recreational purposes. Please respect the rules and community guidelines of the ftplace instance you're connecting to.
