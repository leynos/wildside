SHELL := bash
KUBE_VERSION ?= 1.31.0

define ensure_tool
	@command -v $(1) >/dev/null 2>&1 || { \
	  printf "Error: '%s' is required, but not installed\n" "$(1)" >&2; \
	  exit 1; \
	}
endef

# Prefer PATH-installed tools but fall back to `bun x` for ephemeral runs.
#
# Parameters:
#   $(1) - command name to execute (e.g. `biome`)
#   $(2) - arguments passed to the command
#   $(3) - optional npm package spec for the Bun fallback
define exec_or_bunx
	if command -v $(1) >/dev/null 2>&1; then \
	  $(1) $(2); \
	else \
	  bun x $(if $(3),--package=$(3) ,)$(1) $(2); \
	fi
endef

RUSTFLAGS_STRICT := -D warnings
RUST_FLAGS ?= $(RUSTFLAGS_STRICT)
RUST_FLAGS_ENV := RUSTFLAGS="$(RUST_FLAGS)"
RUSTDOC_FLAGS ?= --cfg docsrs -D warnings

ASYNCAPI_CLI_VERSION ?= 3.4.2
REDOCLY_CLI_VERSION ?= 2.1.0
ORVAL_VERSION ?= 7.18.0
BIOME_VERSION ?= 2.3.1
TSC_VERSION ?= 5.9.2
MARKDOWNLINT_CLI2_VERSION ?= 0.14.0
OPENAPI_SPEC ?= spec/openapi.json

# Place one consolidated PHONY declaration near the top of the file
.PHONY: all clean be fe fe-build openapi gen docker-up docker-down fmt lint test typecheck deps lockfile \
        check-fmt markdownlint markdownlint-docs mermaid-lint nixie yamllint audit \
        lint-asyncapi lint-openapi lint-makefile lint-actions lint-architecture workspace-sync

workspace-sync:
	./scripts/sync_workspace_members.py

all: check-fmt lint test

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

openapi: workspace-sync
	$(call ensure_tool,jq)
	mkdir -p $(dir $(OPENAPI_SPEC))
	./scripts/generate_openapi.sh $(OPENAPI_SPEC)

gen: openapi
	cd frontend-pwa && $(call exec_or_bunx,orval,--config orval.config.cjs,orval@$(ORVAL_VERSION))

docker-up:
	cd deploy && docker compose up --build -d

docker-down:
	cd deploy && docker compose down

fmt: workspace-sync
	cargo fmt --all
	$(call exec_or_bunx,biome,format --write frontend-pwa packages,@biomejs/biome@$(BIOME_VERSION))

lint: workspace-sync
	RUSTDOCFLAGS="$(RUSTDOC_FLAGS)" cargo doc --workspace --no-deps
	cargo clippy --workspace --all-targets --all-features -- $(RUST_FLAGS)
	$(MAKE) lint-architecture
	$(call exec_or_bunx,biome,ci --formatter-enabled=true --reporter=github frontend-pwa packages,@biomejs/biome@$(BIOME_VERSION))
	$(MAKE) lint-asyncapi lint-openapi lint-makefile lint-actions

lint-architecture:
	$(RUST_FLAGS_ENV) cargo run -p architecture-lint --quiet

# Lint AsyncAPI spec if present. Split to keep `lint` target concise per checkmake rules.
lint-asyncapi:
	if [ -f spec/asyncapi.yaml ]; then \
	  bun x --package=@asyncapi/cli@$(ASYNCAPI_CLI_VERSION) asyncapi validate spec/asyncapi.yaml; \
	fi

# Lint OpenAPI spec with Redocly CLI
define LINT_OPENAPI_CMD
$(call ensure_tool,python3)
@if ! grep -F -q "$(OPENAPI_SPEC):" .redocly.lint-ignore.yaml; then \
	echo "OpenAPI ignore file missing entry for $(OPENAPI_SPEC)" >&2; \
	exit 1; \
fi
@python3 scripts/check_redoc_ignore.py
bun x --package=@redocly/cli@$(REDOCLY_CLI_VERSION) redocly lint $(OPENAPI_SPEC)
endef

lint-openapi:
	$(LINT_OPENAPI_CMD)

# Validate Makefile style and structure
lint-makefile:
	command -v checkmake >/dev/null || { echo "checkmake is not installed" >&2; exit 1; }
	command -v mbake >/dev/null || { echo "mbake is not installed" >&2; exit 1; }
	checkmake Makefile
	mbake validate Makefile

define LINT_ACTIONS_CMD
$(call ensure_tool,yamllint)
$(call ensure_tool,action-validator)
$(call ensure_tool,actionlint)
@if [ ! -d .github/actions ]; then \
  echo "No composite actions found; skipping lint-actions"; \
else \
  find .github/actions -name 'action.yml' -print0 | xargs -0 -r yamllint; \
  while IFS= read -r -d '' action; do \
    echo "$$action:"; \
    action-validator "$$action"; \
  done < <(find .github/actions -name 'action.yml' -print0); \
fi
@if [ ! -d .github/workflows ]; then \
  echo "No workflows found; skipping workflow lint"; \
else \
  find .github/workflows \( -name '*.yml' -o -name '*.yaml' \) -print0 | xargs -0 -r yamllint; \
  find .github/workflows \( -name '*.yml' -o -name '*.yaml' \) -print0 | xargs -0 -r actionlint; \
fi
endef

lint-actions:
	$(LINT_ACTIONS_CMD)

PG_WORKER_PATH ?= $(CURDIR)/target/pg_worker

test: workspace-sync deps typecheck prepare-pg-worker
	PG_EMBEDDED_WORKER=$(PG_WORKER_PATH) $(RUST_FLAGS_ENV) cargo nextest run --workspace --all-targets --all-features
	pnpm -r --if-present --silent run test

.PHONY: prepare-pg-worker
prepare-pg-worker:
	$(RUST_FLAGS_ENV) cargo build -p backend --bin pg_worker
	install -m 0755 target/debug/pg_worker $(PG_WORKER_PATH)
	find $(dir $(PG_WORKER_PATH)) -maxdepth 1 -type d -name 'pg-embed-*' -exec rm -rf {} +

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
	@[ -f $(PNPM_LOCK_FILE) ] || { echo "Error: pnpm-lock.yaml missing. Generate it locally (pnpm i) and commit it."; exit 1; }
	pnpm install --frozen-lockfile
	@rm -f node_modules/.pnpm-install-*
	@touch $@

typecheck: deps ; for dir in $(TS_WORKSPACES); do $(call exec_or_bunx,tsc,--noEmit -p $$dir/tsconfig.json,typescript@$(TSC_VERSION)) || exit 1; done

audit: deps
	pnpm -r install
	pnpm -r --if-present run audit
	pnpm run audit

lockfile:
	pnpm install --lockfile-only
	git diff --exit-code pnpm-lock.yaml

define CHECK_FMT_CMD
@if cargo fmt --help | grep -q -- '--workspace'; then \
	cargo fmt --workspace --all -- --check; \
else \
	cargo fmt --all -- --check; \
fi
$(call exec_or_bunx,biome,ci --formatter-enabled=true --reporter=github frontend-pwa packages,@biomejs/biome@$(BIOME_VERSION))
endef

check-fmt:
	$(CHECK_FMT_CMD)

markdownlint:
	$(call exec_or_bunx,markdownlint-cli2,'**/*.md',markdownlint-cli2@$(MARKDOWNLINT_CLI2_VERSION))

nixie:
	bun install
	bun scripts/install-mermaid-browser.mjs
	# CI currently requires --no-sandbox; remove once nixie supports
	# environment variable control for this option
	nixie --no-sandbox

yamllint:
	command -v helm >/dev/null && command -v yamllint >/dev/null && command -v yq >/dev/null
	set -o pipefail; helm template wildside ./deploy/charts/wildside --kube-version $(KUBE_VERSION) | yamllint -f parsable -
