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
- Runs as an Istio `WasmPlugin` or standalone Envoy WASM filter

**Configuration:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `capture_request` | bool | `true` | Log request bodies |
| `capture_response` | bool | `true` | Log response bodies |
| `services` | string[] | `[]` | gRPC services to capture (empty = all) |
| `methods` | string[] | `[]` | gRPC methods to capture (empty = all) |
| `max_payload_bytes` | int | `0` | Max payload bytes to decode (0 = unlimited) |

## Building

**Prerequisites:** Rust toolchain with the `wasm32-unknown-unknown` target.

```sh
rustup target add wasm32-unknown-unknown
```

**Build:**

```sh
cd grpc-proto-extract
make build
```

The compiled plugin is output to `target/wasm32-unknown-unknown/release/grpc_proto_extract.wasm`.

**Other targets:**

```sh
make test    # Run tests
make lint    # Run clippy
make fmt     # Check formatting
make clean   # Clean build artifacts
```

## Deployment

### Istio WasmPlugin

A sample manifest is provided in [`grpc-proto-extract/deploy/wasmplugin.yaml`](grpc-proto-extract/deploy/wasmplugin.yaml). Update the `url` field to point to your OCI registry:

```yaml
url: oci://ghcr.io/aburan28/wasm-envoy-experiments/grpc-proto-extract:latest
```

Then apply:

```sh
kubectl apply -f grpc-proto-extract/deploy/wasmplugin.yaml
```

## CI/CD

- **CI** (`.github/workflows/ci.yml`) — Runs lint, test, and build on every push/PR to `main`. Uploads the `.wasm` binary as a GitHub Actions artifact.
- **Release** (`.github/workflows/release.yml`) — On version tags (`v*`), builds the plugin, pushes to GHCR as an OCI artifact, and creates a GitHub Release with the `.wasm` binary attached.

To create a release:

```sh
git tag v0.1.0
git push origin v0.1.0
```
