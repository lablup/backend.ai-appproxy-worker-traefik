# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Traefik plugin for Backend.AI AppProxy worker with dual implementations: a Rust WASM plugin and a Go native plugin. Both provide session tracking and lifecycle management for Backend.AI applications through Traefik middleware.

## Architecture

This is a Cargo workspace with the following structure:

### Workspace Members
- **`proxy/`**: Main WASM plugin binary (`appproxy-traefik-proxy`)
- **`lib/`**: Shared library (`appproxy_traefik_lib`) containing common functionality
- **`traefik_wasm_api/`**: Custom Rust library providing bindings to Traefik's WASM HTTP handler interface
- **`go/`**: Go implementation of the plugin (`appproxy_traefik_plugin_go`)

### Core Components
- **Main Plugin Logic** (`proxy/src/main.rs`): Contains the main HTTP request/response handlers with authentication logic for JWT tokens and cookies
- **Shared Library** (`lib/src/`): Common modules used by both implementations:
  - `lib/src/jwt.rs`: JWT token validation and decoding
  - `lib/src/cookie.rs`: Cookie parsing and validation
  - `lib/src/dto.rs`: Data transfer objects and configuration structures
  - `lib/src/utils.rs`: Utility functions
- **Go Plugin** (`go/plugin.go`): Native Go implementation with Unix socket communication for session tracking

### Key Architectural Patterns
- **Rust WASM Plugin**: Exports `handle_request` and `handle_response` functions that Traefik calls
- **Go Plugin**: Implements standard Go middleware pattern with `ServeHTTP`
- Both implementations support WebSocket connection tracking and lifecycle management
- Last-used time tracking for circuits and sessions via file I/O (Rust) or Unix socket (Go)
- Shared configuration structure between implementations

## Development Commands

### Build Commands
```bash
# Development build (builds WASM plugin)
make build

# Production release build
make release

# Clean build artifacts
make clean
```

### Setup and Dependencies
```bash
# Install required Rust target
make setup
# Equivalent to: rustup target add wasm32-wasip1
```

### Testing and Development
```bash
# Run with Docker Compose (includes Traefik and test service)
make docker

# Test HTTP requests against running instance  
make http
# Equivalent to: curl -sS -D - 127.0.0.1/foo -o /dev/null

# Debug WASM output
make debug  # Uses wasm2wat to inspect generated WASM
```

### Linting and Formatting
```bash
# Format code (used in CI)
cargo fmt --target wasm32-wasip1

# Check code without building
cargo check --target wasm32-wasip1 --no-default-features
```

## Build Configuration

- **Workspace**: Uses Cargo workspace with shared version, authors, and metadata
- **Target**: `wasm32-wasip1` (WASI preview 1) for Rust WASM plugin
- **Crate Types**: `cdylib` and `rlib` for libraries to support both WASM and native builds
- **Profile**: Release builds use `opt-level = "s"` for size optimization
- **Plugin Output**: `appproxy-traefik-proxy.wasm` (changed from previous `appproxy-traefik.wasm`)

## Plugin Integration

The built plugins are packaged as:
- **WASM Plugin**: `pkg/backend.ai/appproxy-traefik-plugin/plugin.wasm` with `.traefik.yml`
- **Go Plugin**: `pkg/backend.ai/appproxy-traefik-plugin-go/` directory structure
- The CI builds both structures automatically and packages them for release

## Development Notes

- **Dual Implementation**: Both Rust WASM and Go native implementations available
- **Session Tracking**: Go version uses Unix socket communication, Rust version uses file I/O
- **WebSocket Support**: Both implementations handle WebSocket lifecycle (mark active/inactive)
- **Shared Logic**: Common authentication and utility logic extracted to `lib/` workspace member
- The project structure has been refactored from a single crate to a multi-crate workspace