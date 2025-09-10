SHELL := bash
KUBE_VERSION ?= 1.31.0
# Supported DigitalOcean Kubernetes release. Update to a current patch from
# the 1.33.x, 1.32.x or 1.31.x series as listed in the DigitalOcean docs.
DOKS_KUBERNETES_VERSION ?= 1.33.9-do.0

define ensure_tool
	@command -v $(1) >/dev/null 2>&1 || { \
	  printf "Error: '%s' is required, but not installed\n" "$(1)" >&2; \
	  exit 1; \
	}
endef

ASYNCAPI_CLI_VERSION ?= 3.4.2
.PHONY: all clean be fe fe-build openapi gen docker-up docker-down fmt lint test typecheck deps \
        check-fmt markdownlint markdownlint-docs mermaid-lint nixie yamllint audit \
        lint-asyncapi lint-openapi lint-makefile
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
	$(MAKE) lint-asyncapi
	$(MAKE) lint-openapi
	$(MAKE) lint-makefile

# Lint AsyncAPI spec if present. Split to keep `lint` target concise per checkmake rules.
lint-asyncapi:
	if [ -f spec/asyncapi.yaml ]; then bun x -y @asyncapi/cli@$(ASYNCAPI_CLI_VERSION) validate spec/asyncapi.yaml; fi

# Lint OpenAPI spec with Redocly CLI
lint-openapi:
	bun x -y @redocly/cli@latest lint spec/openapi.json

# Validate Makefile style and structure
lint-makefile:
	command -v checkmake >/dev/null || { echo "checkmake is not installed" >&2; exit 1; }
	command -v mbake >/dev/null || { echo "mbake is not installed" >&2; exit 1; }
	checkmake Makefile
	mbake validate Makefile

test: deps
	RUSTFLAGS="-D warnings" cargo test --manifest-path backend/Cargo.toml --all-targets --all-features
	pnpm -r --if-present --silent run test

TS_WORKSPACES := frontend-pwa packages/tokens packages/types
PNPM_LOCK_FILE := pnpm-lock.yaml
PNPM_LOCK_HASH := $(shell \
  if [ -f $(PNPM_LOCK_FILE) ]; then \
    if command -v sha256sum >/dev/null 2>&1; then \
      sha256sum $(PNPM_LOCK_FILE) | awk '{print $$1}'; \
    else \
      shasum -a 256 $(PNPM_LOCK_FILE) | awk '{print $$1}'; \
    fi; \
  else \
    echo "MISSING_LOCKFILE"; \
  fi)
NODE_MODULES_STAMP := node_modules/.pnpm-install-$(PNPM_LOCK_HASH)

deps: $(NODE_MODULES_STAMP)

$(NODE_MODULES_STAMP): $(PNPM_LOCK_FILE) package.json
	[ -f $(PNPM_LOCK_FILE) ] || { echo "$(PNPM_LOCK_FILE) missing"; exit 1; }
	pnpm install --frozen-lockfile
	touch $@

typecheck: deps ; for dir in $(TS_WORKSPACES); do bun x tsc --noEmit -p $$dir/tsconfig.json || exit 1; done

audit: deps
	pnpm -r --if-present run audit
	pnpm audit --recursive

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
	[ ! -f deploy/k8s/overlays/production/patch-helmrelease-values.yaml ] || \
        (set -o pipefail; helm template wildside ./deploy/charts/wildside -f <(yq e '.spec.values' deploy/k8s/overlays/production/patch-helmrelease-values.yaml) --kube-version $(KUBE_VERSION) | yamllint -f parsable -)

.PHONY: conftest tofu doks-test
conftest:
	$(call ensure_tool,conftest)

tofu:
	$(call ensure_tool,tofu)

doks-test:
	tofu fmt -check infra/modules/doks
	tofu -chdir=infra/modules/doks/examples/basic init
	tofu -chdir=infra/modules/doks/examples/basic validate
	command -v tflint >/dev/null
	cd infra/modules/doks && tflint --init && tflint --config .tflint.hcl --version && tflint --config .tflint.hcl
	conftest test infra/modules/doks --policy infra/modules/doks/policy --ignore ".terraform"
	cd infra/modules/doks/tests && go test -v
	# Optional: surface "changes pending" in logs without failing CI
	tofu -chdir=infra/modules/doks/examples/basic plan -detailed-exitcode \
	-var cluster_name=test \
	-var region=nyc1 \
        -var kubernetes_version=$(DOKS_KUBERNETES_VERSION) \
	-var 'node_pools=[{"name"="default","size"="s-2vcpu-2gb","node_count"=2,"auto_scale"=false,"min_nodes"=2,"max_nodes"=2}]' \
	|| test $$? -eq 2
	$(MAKE) doks-policy

.PHONY: doks-policy
doks-policy: conftest tofu
	tofu -chdir=infra/modules/doks/examples/basic plan -out=tfplan.binary -detailed-exitcode \
	-var cluster_name=test \
	-var region=nyc1 \
        -var kubernetes_version=$(DOKS_KUBERNETES_VERSION) \
	-var 'node_pools=[{"name"="default","size"="s-2vcpu-2gb","node_count"=2,"auto_scale"=false,"min_nodes"=2,"max_nodes"=2}]' \
	|| test $$? -eq 2
	tofu -chdir=infra/modules/doks/examples/basic show -json tfplan.binary > infra/modules/doks/examples/basic/plan.json
	conftest test infra/modules/doks/examples/basic/plan.json --policy infra/modules/doks/policy
