SHELL := bash
KUBE_VERSION ?= 1.31.0
ASYNCAPI_CLI_VERSION ?= 3.4.2
.PHONY: all clean be fe fe-build openapi gen docker-up docker-down fmt lint test typecheck deps \
        check-fmt markdownlint markdownlint-docs mermaid-lint nixie yamllint audit
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
	command -v checkmake >/dev/null || { echo "checkmake is not installed" >&2; exit 1; }
	command -v mbake >/dev/null || { echo "mbake is not installed" >&2; exit 1; }
	if [ -f spec/asyncapi.yaml ]; then bun x -y @asyncapi/cli@$(ASYNCAPI_CLI_VERSION) validate spec/asyncapi.yaml; fi
	bun x -y @redocly/cli@latest lint spec/openapi.json
	checkmake Makefile
	mbake validate Makefile

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

audit:
	npm run audit

check-fmt:
	cargo fmt --manifest-path backend/Cargo.toml --all -- --check
	bun x biome format

markdownlint:
	find . \
	  \( -path './backend/target' -o -path './target' -o \
	     -path './node_modules' -o -path '*/node_modules' -o \
	     -path '*/.git' \) -prune -o -type f -name '*.md' -print0 | \
	     xargs -0 -- markdownlint

nixie:
	# CI currently requires --no-sandbox; remove once nixie supports
	# environment variable control for this option
	nixie --no-sandbox

yamllint:
	command -v helm >/dev/null
	command -v yamllint >/dev/null
	command -v yq >/dev/null
	set -o pipefail; helm template wildside ./deploy/charts/wildside --kube-version $(KUBE_VERSION) | yamllint -f parsable -
	if [ -f deploy/k8s/overlays/production/patch-helmrelease-values.yaml ] && yq e -e '.spec.values' deploy/k8s/overlays/production/patch-helmrelease-values.yaml >/dev/null; then set -o pipefail; helm template wildside ./deploy/charts/wildside -f <(yq e '.spec.values' deploy/k8s/overlays/production/patch-helmrelease-values.yaml) --kube-version $(KUBE_VERSION) | yamllint -f parsable -; fi
