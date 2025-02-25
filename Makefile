prog := plugin

build:
	make release_cleanup
	RUSTFLAGS="--cfg wasmedge --cfg tokio_unstable" cargo build --target wasm32-wasip1
	mkdir -p pkg/backend.ai/appproxy-traefik-plugin
	cp .traefik.yml pkg/backend.ai/appproxy-traefik-plugin
	cp -r go pkg/backend.ai/appproxy-traefik-plugin-go
	cp target/wasm32-wasip1/debug/appproxy-traefik-proxy.wasm pkg/backend.ai/appproxy-traefik-plugin/$(prog).wasm

release:
	make release_cleanup
	RUSTFLAGS="--cfg wasmedge --cfg tokio_unstable" cargo build --target wasm32-wasip1 --release
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

docker:
	make build
	docker compose up

setup:
	rustup target add wasm32-wasip1
	cargo check --target wasm32-wasip1 --no-default-features

http:
	curl -sS -D - 127.0.0.1/foo -o /dev/null

debug:
	make build
	wasm2wat pkg/traefik-auth.wasm | less


cargo:
	cargo publish --manifest-path libs/traefik_wasm_api/Cargo.toml --target wasm32-unknown-unknown
