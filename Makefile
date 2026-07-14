prog := plugin

build:
	make release_cleanup
	cargo build --target wasm32-wasip1
	mkdir -p pkg/backend.ai/appproxy-traefik-plugin
	cp .traefik.yml pkg/backend.ai/appproxy-traefik-plugin
	cp -r go pkg/backend.ai/appproxy-traefik-plugin-go
	cp target/wasm32-wasip1/debug/appproxy-traefik-proxy.wasm pkg/backend.ai/appproxy-traefik-plugin/$(prog).wasm

release:
	make release_cleanup
	cargo build --target wasm32-wasip1 --release
	mkdir -p pkg/backend.ai/appproxy-traefik-plugin
	cp .traefik.yml pkg/backend.ai/appproxy-traefik-plugin
	cp -r go pkg/backend.ai/appproxy-traefik-plugin-go
	cp target/wasm32-wasip1/release/appproxy-traefik-proxy.wasm pkg/backend.ai/appproxy-traefik-plugin/$(prog).wasm

release_cleanup:
	rm -rf pkg/
	mkdir pkg

clean:
	rm -rf pkg/
	rm -rf target/

setup:
	rustup target add wasm32-wasip1
	cargo check --target wasm32-wasip1 --no-default-features

debug:
	make build
	wasm2wat pkg/backend.ai/appproxy-traefik-plugin/$(prog).wasm | less

cargo:
	cargo publish --manifest-path traefik_wasm_api/Cargo.toml --target wasm32-unknown-unknown
