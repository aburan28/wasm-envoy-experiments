# wasm-envoy-experiments

WASM plugins for Envoy/Istio built with Rust and the [proxy-wasm](https://github.com/proxy-wasm/proxy-wasm-rust-sdk) SDK.

## Plugins

### grpc-proto-extract

An Envoy HTTP filter that intercepts gRPC traffic, extracts protobuf payloads, and logs decoded field information — **without requiring `.proto` schemas**. Useful for debugging and observability of gRPC services in an Istio mesh.

**Features:**
- Schema-less protobuf decoding (extracts field numbers, wire types, and values)
- Configurable capture of request and/or response bodies
- Filter by gRPC service and method names
- Configurable payload size limits

**Configuration:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `capture_request` | bool | `true` | Log request bodies |
| `capture_response` | bool | `true` | Log response bodies |
| `services` | string[] | `[]` | gRPC services to capture (empty = all) |
| `methods` | string[] | `[]` | gRPC methods to capture (empty = all) |
| `max_payload_bytes` | int | `0` | Max payload bytes to decode (0 = unlimited) |

### response-capture

An Envoy HTTP filter that captures HTTP response metadata and bodies, logging them in JSON or plain text format. Useful for auditing, debugging, and response inspection across any HTTP service.

**Features:**
- Capture response headers and bodies
- Filter by HTTP status code and path prefix
- Configurable body size limits
- JSON or plain text output format
- Adds a tag header (`x-response-captured`) to captured responses

**Configuration:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `capture_headers` | bool | `true` | Log response headers |
| `capture_body` | bool | `true` | Log response body |
| `status_codes` | int[] | `[]` | HTTP status codes to capture (empty = all) |
| `path_prefixes` | string[] | `[]` | Path prefixes to capture (empty = all) |
| `max_body_bytes` | int | `0` | Max body bytes to capture (0 = unlimited) |
| `output_format` | string | `"json"` | Log format: `"json"` or `"plain"` |
| `capture_tag_header` | string | `"x-response-captured"` | Header added to captured responses |

## Building

**Prerequisites:** Rust toolchain with the `wasm32-unknown-unknown` target.

```sh
rustup target add wasm32-unknown-unknown
```

**Build a plugin:**

```sh
cd grpc-proto-extract   # or response-capture
make build
```

**Other targets:**

```sh
make test    # Run tests
make lint    # Run clippy
make fmt     # Check formatting
make clean   # Clean build artifacts
```

## Deployment

### Istio WasmPlugin

Sample manifests are provided in each plugin's `deploy/` directory. Update the `url` field to point to your OCI registry:

```yaml
url: oci://ghcr.io/aburan28/wasm-envoy-experiments/grpc-proto-extract:latest
url: oci://ghcr.io/aburan28/wasm-envoy-experiments/response-capture:latest
```

Then apply:

```sh
kubectl apply -f grpc-proto-extract/deploy/wasmplugin.yaml
kubectl apply -f response-capture/deploy/wasmplugin.yaml
```

## CI/CD

- **CI** (`.github/workflows/ci.yml`) — Runs lint, test, and build for all plugins on every push/PR to `main`. Uploads `.wasm` binaries as GitHub Actions artifacts. On merge to `main`, automatically creates and pushes a version tag.
- **Release** (`.github/workflows/release.yml`) — On version tags (`v*`), builds all plugins, pushes to GHCR as OCI artifacts, and creates a GitHub Release with `.wasm` binaries attached.

Tags are auto-incremented (patch version) on every push to `main`. To manually release:

```sh
git tag v0.1.0
git push origin v0.1.0
```
