SHELL := bash
KUBE_VERSION ?= 1.31.0
.PHONY: all clean be fe fe-build openapi gen docker-up docker-down fmt lint test typecheck deps \
        check-fmt markdownlint markdownlint-docs mermaid-lint nixie yamllint
all: fmt lint test

clean:
	cargo clean --manifest-path backend/Cargo.toml
	rm -rf frontend-pwa/node_modules packages/tokens/node_modules

be:
	cargo run --manifest-path backend/Cargo.toml

fe:
	# Long-running dev server
	cd frontend-pwa && bun dev

fe-build:
	pushd frontend-pwa && bun install && popd
	cd frontend-pwa && bun run build

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
	bun x biome format --write

lint:
	cargo clippy --manifest-path backend/Cargo.toml --all-targets --all-features -- -D warnings
	bun x biome ci --formatter-enabled=true --reporter=github frontend-pwa packages

test:
	RUSTFLAGS="-D warnings" cargo test --manifest-path backend/Cargo.toml --all-targets --all-features
	# Ensure JavaScript dependencies are present for all workspaces
	npm ci --workspaces || npm install --workspaces
	npm --workspaces run test --if-present --silent --no-audit --no-fund

TS_WORKSPACES := frontend-pwa packages/tokens packages/types
BUN_LOCK_HASH := $(shell sha256sum bun.lock | awk '{print $$1}')
NODE_MODULES_STAMP := node_modules/.bun-install-$(BUN_LOCK_HASH)

deps: $(NODE_MODULES_STAMP)

$(NODE_MODULES_STAMP): bun.lock package.json ; bun install && touch $@

typecheck: deps ; for dir in $(TS_WORKSPACES); do bun x tsc --noEmit -p $$dir/tsconfig.json || exit 1; done

check-fmt:
	cargo fmt --manifest-path backend/Cargo.toml --all -- --check
	bun x biome format

markdownlint:
	find . \
          -path './backend/target' -prune -o \
          -path './target' -prune -o \
          -path './.node_modules' -prune -o \
          -path '*/node_modules' -prune -o \
          -path './.git' -prune -o \
          -path '*/.git' -prune -o \
          -type f -name '*.md' -print0 | xargs -0 -- markdownlint

nixie:
	# CI currently requires --no-sandbox; remove once nixie supports
	# environment variable control for this option
	nixie --no-sandbox

yamllint:
	command -v helm >/dev/null
	command -v yamllint >/dev/null
	command -v yq >/dev/null
	set -o pipefail; helm template wildside ./deploy/charts/wildside --kube-version $(KUBE_VERSION) | yamllint -f parsable -
	[ ! -f deploy/k8s/overlays/production/patch-helmrelease-values.yaml ] || \
	(set -o pipefail; helm template wildside ./deploy/charts/wildside -f <(yq e '.spec.values' deploy/k8s/overlays/production/patch-helmrelease-values.yaml) --kube-version $(KUBE_VERSION) | yamllint -f parsable -)
