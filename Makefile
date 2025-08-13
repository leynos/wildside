.PHONY: be fe openapi gen docker-up docker-down fmt lint test check-fmt markdownlint

be:
	cargo run --manifest-path backend/Cargo.toml

fe:
	cd frontend-pwa && bun dev

openapi:
	# Replace with a bin that prints OpenAPI
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

test:
	RUSTFLAGS="-D warnings" cargo test --manifest-path backend/Cargo.toml --all-targets --all-features

check-fmt:
	cargo fmt --manifest-path backend/Cargo.toml --all -- --check

markdownlint:
	find . -type f -name '*.md' -not -path './target/*' -print0 | xargs -0 -- markdownlint
