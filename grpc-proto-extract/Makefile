.PHONY: build test lint fmt clean

# Use rustup cargo if available (avoids Homebrew rustc missing wasm32 sysroot)
CARGO := $(shell command -v $(HOME)/.cargo/bin/cargo 2>/dev/null || echo cargo)

build:
	$(CARGO) build --target wasm32-unknown-unknown --release
	@ls -lh target/wasm32-unknown-unknown/release/grpc_proto_extract.wasm

test:
	$(CARGO) test

lint:
	$(CARGO) clippy --target wasm32-unknown-unknown -- -D warnings

fmt:
	$(CARGO) fmt --check

clean:
	$(CARGO) clean
