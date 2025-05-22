# Makefile for ftplace-TUI

# Variables
APP_NAME := ftplace_tui
TARGET_DIR := target
DEBUG_BUILD := $(TARGET_DIR)/debug/$(APP_NAME)
RELEASE_BUILD := $(TARGET_DIR)/release/$(APP_NAME)

# Default local server URL for ftplace web app (Nginx proxy)
LOCAL_SERVER_URL := "http://localhost:7979"

# Default target
.PHONY: all
all: build

# Build release version
.PHONY: build
build:
	@echo "Building release version of $(APP_NAME)..."
	cargo build --release
	@echo "Release build complete: $(RELEASE_BUILD)"

# Build debug version (often faster, used by run)
.PHONY: build-debug
build-debug:
	@echo "Building debug version of $(APP_NAME)..."
	cargo build
	@echo "Debug build complete: $(DEBUG_BUILD)"

# Run debug version
# You will need to provide tokens for most operations.
.PHONY: run
run: build-debug
	@echo "Running $(APP_NAME) (debug build)..."
	@echo "Provide tokens using --access-token YOUR_ACCESS_TOKEN --refresh-token YOUR_REFRESH_TOKEN"
	@echo "Or set FTPLACE_ACCESS_TOKEN and FTPLACE_REFRESH_TOKEN environment variables."
	$(DEBUG_BUILD)

# Run debug version against local server, reminding about tokens
# Example: make run-local FTPLACE_ACCESS_TOKEN="your_access_token" FTPLACE_REFRESH_TOKEN="your_refresh_token"
.PHONY: run-local
run-local: build-debug
	@echo "Running $(APP_NAME) against local server: $(LOCAL_SERVER_URL)"
	@echo "Ensure your ftplace web server is running locally."
	@echo "Ensure FTPLACE_ACCESS_TOKEN and FTPLACE_REFRESH_TOKEN are set in your environment or pass them as arguments."
	$(DEBUG_BUILD) --base-url $(LOCAL_SERVER_URL)

# Clean build artifacts
.PHONY: clean
clean:
	@echo "Cleaning build artifacts..."
	cargo clean

# Check code without building
.PHONY: check
check:
	@echo "Checking code..."
	cargo check

# Format code
.PHONY: fmt
fmt:
	@echo "Formatting code..."
	cargo fmt

# Lint code with Clippy
.PHONY: clippy
clippy:
	@echo "Linting with Clippy..."
	cargo clippy

# Help target
.PHONY: help
help:
	@echo "Available targets:"
	@echo "  all          - Build the release version (default)"
	@echo "  build        - Build the release version"
	@echo "  build-debug  - Build the debug version"
	@echo "  run          - Run the debug version (prompts for tokens or uses env vars)"
	@echo "  run-local    - Run the debug version against local server ($(LOCAL_SERVER_URL)) (prompts for tokens or uses env vars)"
	@echo "  clean        - Clean build artifacts"
	@echo "  check        - Check code"
	@echo "  fmt          - Format code"
	@echo "  clippy       - Lint code with Clippy"
	@echo "To pass tokens to 'run' or 'run-local', either set environment variables FTPLACE_ACCESS_TOKEN and FTPLACE_REFRESH_TOKEN,"
	@echo "or append arguments directly: e.g., make run-local ARGS='--access-token X --refresh-token Y' (less direct with make),"
	@echo "or simply run the compiled binary with arguments: e.g., ./target/debug/ftplace_tui --base-url $(LOCAL_SERVER_URL) --access-token X --refresh-token Y" 