.PHONY: all clean be fe openapi gen docker-up docker-down fmt lint test check-fmt markdownlint markdownlint-docs mermaid-lint

all: fmt lint test

clean:
	cargo clean --manifest-path backend/Cargo.toml
	rm -rf frontend-pwa/node_modules packages/tokens/node_modules

be:
	cargo run --manifest-path backend/Cargo.toml

fe:
	# Long-running dev server
	cd frontend-pwa && bun dev

openapi:
	# Replace with a bin that prints OpenAPI
	mkdir -p spec
	curl -s http://localhost:8080/api-docs/openapi.json > spec/openapi.json

gen:
	cd frontend-pwa && bunx orval --config orval.config.yaml

docker-up:
	cd deploy && docker compose up --build -d

docker-down:
	cd deploy && docker compose down

fmt:
	cargo fmt --manifest-path backend/Cargo.toml --all


lint:
	cargo clippy --manifest-path backend/Cargo.toml --all-targets --all-features -- -D warnings
	npx biome lint frontend-pwa packages

test:
	RUSTFLAGS="-D warnings" cargo test --manifest-path backend/Cargo.toml --all-targets --all-features

check-fmt:
	cargo fmt --manifest-path backend/Cargo.toml --all -- --check

markdownlint:
	find . -type f -name '*.md' -not -path './target/*' -print0 | xargs -0 -- markdownlint

markdownlint-docs:
	markdownlint docs/repository-structure.md

mermaid-lint:
	npx --yes @mermaid-js/mermaid-cli -i docs/repository-structure.md -o /tmp/diagram.svg -p mmdc-puppeteer.json
