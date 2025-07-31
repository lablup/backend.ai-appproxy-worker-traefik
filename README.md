# Backend.AI AppProxy Traefik Plugin

A Traefik middleware plugin for Backend.AI AppProxy worker that provides authentication, session tracking, and lifecycle management for Backend.AI applications. This plugin is available in two implementations: Rust WASM and Go native.

## Purpose

This project provides two complementary Traefik plugins that serve different purposes in the Backend.AI AppProxy ecosystem:

### WASM Plugin (Rust Implementation)
The WASM plugin handles **authentication and authorization** for Backend.AI applications:

- **Interactive Session Authentication**: Validates HMAC-based cookies for user-initiated interactive sessions (Jupyter notebooks, RStudio, etc.)
- **Inference Endpoint Authentication**: Validates JWT tokens for programmatic API access to inference endpoints
- **Multi-Protocol Support**: Supports HTTP, gRPC, HTTP/2, TCP, and preopen protocols
- **Flexible Frontend Modes**: Works with both wildcard domain and port-based routing
- **WebSocket Support**: Handles WebSocket connection lifecycle with active/inactive state tracking

### Go Plugin (Native Implementation)
The Go plugin focuses on **session tracking and monitoring**:

- **Route Access Tracking**: Records last-accessed timestamps for each Traefik route
- **Circuit Lifecycle Management**: Tracks when circuits become active or inactive
- **Session Monitoring**: Monitors session usage patterns for resource management
- **Unix Socket Communication**: Communicates with Backend.AI AppProxy via Unix domain sockets for efficient data exchange

### Combined Functionality
Together, these plugins ensure proper authentication for Backend.AI applications while maintaining comprehensive session activity tracking for resource management, billing, and automatic cleanup of idle resources.

**Important**: This plugin is designed to work in conjunction with [Backend.AI AppProxy](https://github.com/lablup/backend.ai/tree/main/src/ai/backend/appproxy). The AppProxy component automatically manages the plugin configuration, circuit information, and authentication secrets. The plugin cannot function independently without the AppProxy system.

## Architecture

### Project Structure

This is a Cargo workspace containing:

```
├── proxy/           # Main WASM plugin binary (appproxy-traefik-proxy)
├── lib/             # Shared library (appproxy_traefik_lib)
├── traefik_wasm_api/ # Traefik WASM HTTP handler bindings
├── go/              # Go native plugin implementation
├── pkg/             # Build output directory
└── target/          # Rust build artifacts
```

### Core Components

#### Rust WASM Implementation (`proxy/`)
- **Main Handler** (`proxy/src/main.rs`): Exports `handle_request` and `handle_response` functions for Traefik WASM runtime
- **Configuration**: Lazy-loaded plugin configuration from Traefik with circuit information, JWT secrets, and cookie settings
- **Authentication Logic**: 
  - **Interactive Sessions**: Validates HMAC-based cookies for user sessions accessing Jupyter, RStudio, and other interactive applications
  - **Inference Endpoints**: Validates JWT tokens for programmatic API access to model inference endpoints
  - **Public Access**: Allows unauthenticated access when circuits are marked as public

#### Shared Library (`lib/`)
- **JWT Module** (`lib/src/jwt.rs`): JWT token decoding and validation
- **Cookie Module** (`lib/src/cookie.rs`): Cookie parsing and HMAC validation
- **DTO Module** (`lib/src/dto.rs`): Data structures for circuits, routes, and configuration
- **Utils Module** (`lib/src/utils.rs`): Common utility functions

#### Go Native Implementation (`go/`)
- **Standard Middleware** (`go/plugin.go`): Implements Go's `http.Handler` interface
- **Route Tracking Logic**: Records last-accessed timestamps for each Traefik route that passes through the middleware
- **Circuit State Management**: Tracks active/inactive state of circuits, particularly for WebSocket connections
- **Unix Socket Communication**: Communicates with Backend.AI AppProxy via Unix domain sockets to report usage statistics
- **Lightweight Design**: Focuses purely on tracking without authentication logic for optimal performance

#### Traefik WASM API (`traefik_wasm_api/`)
- Custom Rust bindings for Traefik's WASM HTTP handler interface
- Provides functions for header manipulation, body reading/writing, logging, and configuration access

### Data Flow

#### WASM Plugin (Authentication Flow)
1. **Request Processing**:
   - Plugin receives HTTP request through Traefik
   - Determines authentication method based on circuit configuration:
     - **Interactive sessions** (`user_id` present): Validates HMAC cookie
     - **Inference endpoints** (`endpoint_id` present): Validates JWT token
     - **Public circuits**: Allows access without authentication
   - Detects WebSocket upgrade requests for connection tracking
   - Returns execution control to Traefik (continue or stop with 401 if auth fails)

2. **Response Processing**:
   - Handles WebSocket connection cleanup
   - Marks circuits as inactive when WebSocket connections close

#### Go Plugin (Tracking Flow)
1. **Request Processing**:
   - Records timestamp when route is accessed
   - Identifies WebSocket connections for state tracking
   - Communicates with AppProxy via Unix socket to report usage

2. **Response Processing**:
   - Updates circuit state (active/inactive) based on connection type
   - Reports final usage statistics to AppProxy

3. **Configuration Structure**:
   ```json
   {
     "circuit": "{ JSON circuit configuration }",
     "jwt_secret": "base64-encoded-secret",
     "permit_hash_secret": "hmac-secret",
     "permit_cookie_name": "cookie-name"
   }
   ```

## Installation

### Prerequisites

- **Backend.AI AppProxy**: This plugin requires [Backend.AI AppProxy](https://github.com/lablup/backend.ai/tree/main/src/ai/backend/appproxy) to be installed and configured
- **Rust Toolchain**: 1.70+ with `wasm32-wasip1` target
- **Go**: 1.19+ (for Go implementation)
- **Traefik**: v3.0+ with WASM plugin support
- **Docker & Docker Compose** (for testing)

### Setup Development Environment

```bash
# Clone the repository
git clone https://github.com/lablup/backend.ai-appproxy-worker-traefik.git
cd backend.ai-appproxy-worker-traefik

# Install Rust WASM target
make setup
# Equivalent to: rustup target add wasm32-wasip1
```

### Building the Plugin

#### Rust WASM Plugin
```bash
# Development build
make build

# Production build (optimized for size)
make release

# Clean build artifacts
make clean
```

#### Go Plugin
The Go plugin is built automatically when using the provided Docker Compose setup. Note that the Go plugin is typically deployed separately from the WASM plugin, as they serve different purposes - the WASM plugin for authentication and the Go plugin for usage tracking.

### Installation Methods

#### Method 1: Local Plugin (Development)

1. Build the plugin:
   ```bash
   make release
   ```

2. Configure Traefik with local plugin:
   ```yaml
   # traefik.yml
   experimental:
     localPlugins:
       traefik-appproxy:
         modulename: github.com/lablup/backend.ai-appproxy-worker-traefik
   
   entryPoints:
     web:
       address: ":80"
   ```

3. Mount plugin files:
   ```bash
   # WASM Plugin
   cp pkg/backend.ai/appproxy-traefik-plugin/plugin.wasm /path/to/plugins/
   cp .traefik.yml /path/to/plugins/
   
   # Go Plugin  
   cp -r pkg/backend.ai/appproxy-traefik-plugin-go/ /path/to/plugins/
   ```

#### Method 2: Docker Compose (Testing)

```bash
# Start test environment
make docker

# Test the plugin
make http
```

## Configuration

### Plugin Configuration

**Note**: In production environments, plugin configuration is automatically managed by Backend.AI AppProxy. The AppProxy component dynamically generates and updates the configuration based on circuit information, user sessions, and security settings.

For development and testing purposes, you can manually configure the middleware:

```yaml
# Dynamic configuration
http:
  middlewares:
    appproxy-auth:
      plugin:
        traefik-appproxy:
          circuit: '{"id":"circuit-123","app":"jupyter","protocol":"http","worker":"worker-1","app_mode":"interactive","frontend_mode":"wildcard","envs":{},"open_to_public":false,"session_ids":["session-456"],"route_info":[{"session_id":"session-456","kernel_host":"127.0.0.1","kernel_port":8888,"protocol":"http","traffic_ratio":1.0}],"created_at":"2024-01-01T00:00:00Z","updated_at":"2024-01-01T00:00:00Z"}'
          jwt_secret: "your-jwt-secret-base64"
          permit_hash_secret: "your-hmac-secret"
          permit_cookie_name: "backend_ai_permit"

  routers:
    app-router:
      rule: "Host(`app.example.com`)"
      service: app-service
      middlewares:
        - appproxy-auth
```

### Circuit Configuration Fields

The circuit configuration is automatically generated by Backend.AI AppProxy based on session and application information:

- **`id`**: Unique circuit identifier
- **`app`**: Application type (jupyter, rstudio, etc.)
- **`protocol`**: Communication protocol (http, grpc, http2, tcp, preopen)
- **`app_mode`**: Application mode (interactive, inference)
- **`frontend_mode`**: Routing mode (wildcard, port)
- **`open_to_public`**: Whether authentication is required
- **`user_id`**: User ID for cookie-based authentication
- **`endpoint_id`**: Endpoint ID for JWT-based authentication
- **`session_ids`**: List of associated session IDs
- **`route_info`**: Backend routing information

## Testing

### Unit Testing

```bash
# Test Rust code
cargo test

# Test with specific target
cargo test --target wasm32-wasip1
```

### Integration Testing

```bash
# Start test environment
make docker

# Test HTTP requests
make http

# Manual testing
curl -v http://localhost/foo \
  -H "Authorization: Bearer your-jwt-token"
```

### Debug Mode

```bash
# Build and inspect WASM
make debug

# View detailed logs
docker-compose logs -f traefik
```

### Linting and Formatting

```bash
# Format code
cargo fmt

# Check code quality
cargo clippy

# Check without building
cargo check --target wasm32-wasip1 --no-default-features
```

## Development

### Development Workflow

1. Make changes to source code
2. Run tests: `cargo test`
3. Build plugin: `make build`
4. Test with Docker: `make docker`
5. Verify functionality: `make http`

### Adding New Features

1. **Authentication Methods**: Extend authentication logic in `proxy/src/main.rs` and `lib/src/`
2. **Protocol Support**: Add new protocol variants to `lib/src/dto.rs`
3. **Session Tracking**: Modify session tracking logic for new requirements

### Plugin API

The plugin exports two main functions for Traefik WASM runtime:

- **`handle_request()`**: Called for each incoming request
- **`handle_response(req_ctx: i32, is_error: i32)`**: Called after request processing

## Troubleshooting

### Common Issues

1. **Plugin Not Loading**:
   - Verify WASM file path in Traefik configuration
   - Check Traefik logs for plugin loading errors
   - Ensure `.traefik.yml` is correctly configured

2. **Authentication Failures**:
   - Validate JWT secret configuration
   - Check cookie name matches configuration
   - Verify HMAC secret for cookie validation

3. **Session Tracking Issues**:
   - Confirm session marker file permissions (Rust version)
   - Verify Unix socket accessibility (Go version)
   - Check session ID format in circuit configuration

4. **AppProxy Integration Issues**:
   - Ensure Backend.AI AppProxy is running and properly configured
   - Verify network connectivity between AppProxy and Traefik
   - Check AppProxy logs for configuration generation errors

### Debug Information

Enable debug logging in Traefik:
```yaml
log:
  level: DEBUG
```

The plugin outputs detailed logs for:
- Configuration loading
- Authentication attempts
- Session tracking operations
- WebSocket connection lifecycle

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests for new functionality
5. Run the test suite
6. Submit a pull request

## License

This project is licensed under the terms specified in the repository license file.

## Related Projects

- [Backend.AI](https://github.com/lablup/backend.ai): AI/ML workload orchestration platform
- [Backend.AI AppProxy](https://github.com/lablup/backend.ai/tree/main/src/ai/backend/appproxy): Application proxy component that manages this plugin
- [Traefik](https://github.com/traefik/traefik): Modern reverse proxy and load balancer
- [Traefik WASM Plugin System](https://doc.traefik.io/traefik/plugins/): Traefik's plugin architecture