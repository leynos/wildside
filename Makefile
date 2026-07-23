SHELL := bash
BUN_PATH := $(HOME)/.bun/bin:$(PATH)
CARGO ?= cargo
WHITAKER ?= whitaker
KUBE_VERSION ?= 1.31.0
export PATH := $(HOME)/.cargo/bin:$(HOME)/.bun/bin:$(HOME)/.local/bin:$(HOME)/go/bin:$(CURDIR)/node_modules/.bin:$(PATH)
CARGO_AUDIT_IGNORES := --ignore RUSTSEC-2023-0071

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
REDOCLY_CLI_VERSION ?= 2.19.0
ORVAL_VERSION ?= 7.18.0
BIOME_VERSION ?= 2.3.1
TSC_VERSION ?= 5.9.2
MARKDOWNLINT_CLI2_VERSION ?= 0.14.0
YAMLLINT_VERSION ?= 1.35.1
PATHSPEC_VERSION ?= 1.1.1
RUFF_VERSION ?= 0.15.12
TYPOS_VERSION ?= 1.48.0
UV ?= uv
UV_ENV = UV_CACHE_DIR=.uv-cache UV_TOOL_DIR=.uv-tools
TYPOS_CONFIG_BUILDER_COMMIT := b604f198797fdd36a567dd0f8f07b13f9539b241
TYPOS_CONFIG_BUILDER_SOURCE := git+https://github.com/leynos/typos-config-builder.git@$(TYPOS_CONFIG_BUILDER_COMMIT)
TYPOS_CONFIG_BUILDER := $(UV_ENV) $(UV) tool run --python 3.14 \
	--from "$(TYPOS_CONFIG_BUILDER_SOURCE)" typos-config-builder
SPELLING_PY_SRCS := \
	scripts/typos_rollout_check.py scripts/tests/test_typos_rollout_check.py
SPELLING_PY_TESTS := scripts/tests/test_typos_rollout_check.py
SPELLING_COVERAGE_ARGS := --cov=typos_rollout_check --cov-fail-under=90
PYTHON_NO_BYTECODE_ENV := PYTHONDONTWRITEBYTECODE=1
SPELLING_COVERAGE_FILE ?= /tmp/$(APP)-spelling-helper.coverage
SPELLING_HELPER_PYTEST = PYTHONPATH=scripts $(PYTHON_NO_BYTECODE_ENV) \
	COVERAGE_FILE=$(SPELLING_COVERAGE_FILE) $(UV_ENV) $(UV) run --no-project \
	--python 3.14 --with pathspec==$(PATHSPEC_VERSION) --with pytest==9.0.2 \
	--with pytest-cov==7.0.0 python -m pytest
OPENAPI_SPEC ?= spec/openapi.json

# Place one consolidated PHONY declaration near the top of the file
.PHONY: all clean be fe fe-build openapi gen docker-up docker-down
.PHONY: local-k8s-up local-k8s-down local-k8s-status local-k8s-logs
.PHONY: fmt lint test test-rust test-frontend test-workflow-contracts test-scripts typecheck deps lockfile
.PHONY: lint-specs audit audit-node rust-audit
.PHONY: check-fmt markdownlint markdownlint-docs mermaid-lint nixie yamllint
.PHONY: spelling spelling-phrase-check spelling-config spelling-config-write spelling-helper-test
.PHONY: lint-rust lint-clippy lint-whitaker lint-frontend lint-asyncapi lint-openapi lint-makefile
.PHONY: lint-actions lint-architecture workspace-sync prepare-pg-worker

workspace-sync:
	./scripts/sync_workspace_members.py

all: check-fmt lint test spelling

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

local-k8s-up:
	$(UV) run scripts/local_k8s.py up

local-k8s-down:
	$(UV) run scripts/local_k8s.py down

local-k8s-status:
	$(UV) run scripts/local_k8s.py status

local-k8s-logs:
	$(UV) run scripts/local_k8s.py logs

fmt: workspace-sync
	cargo fmt --all
	$(call exec_or_bunx,biome,format --write frontend-pwa packages,@biomejs/biome@$(BIOME_VERSION))

lint: workspace-sync
	$(MAKE) lint-rust
	$(MAKE) lint-architecture
	$(MAKE) lint-frontend
	$(MAKE) lint-specs
	$(MAKE) lint-makefile lint-actions

lint-rust:
	RUSTDOCFLAGS="$(RUSTDOC_FLAGS)" cargo doc --workspace --no-deps
	$(MAKE) lint-clippy
	$(MAKE) lint-whitaker

# Strict-union of the previous Makefile and CI clippy calls:
#   * --locked (matches CI; fails on lockfile drift)
#   * --workspace (broader than CI's --manifest-path backend/Cargo.toml; covers
#     example-data, pagination, architecture-lint, and other workspace members)
# CI invokes this target so both surfaces stay in lockstep.
lint-clippy:
	cargo clippy --locked --workspace --all-targets --all-features -- $(RUST_FLAGS)

# Whitaker is expected to be on PATH; install with `whitaker-installer`
# (CI bootstraps it in a separate step, pinned via
# WHITAKER_INSTALLER_VERSION). Runs under RUSTFLAGS="-D warnings" so a
# workspace warning fails the lint, matching the clippy gate. CI invokes
# this target so both surfaces stay in lockstep.
lint-whitaker:
	$(RUST_FLAGS_ENV) $(WHITAKER) --all -- --manifest-path Cargo.toml --workspace --all-targets --all-features
	$(RUST_FLAGS_ENV) $(WHITAKER) --all -- --manifest-path backend/Cargo.toml --all-targets --all-features

lint-frontend:
	$(call exec_or_bunx,biome,ci --formatter-enabled=true --reporter=github frontend-pwa packages,@biomejs/biome@$(BIOME_VERSION))


lint-specs: lint-asyncapi lint-openapi

lint-architecture:
	$(RUST_FLAGS_ENV) cargo run -p architecture-lint --quiet

# Lint AsyncAPI spec if present. Split to keep `lint` target concise per checkmake rules.
lint-asyncapi:
	if [ -f spec/asyncapi.yaml ]; then \
	  npm exec --yes --package=@asyncapi/cli@$(ASYNCAPI_CLI_VERSION) -- asyncapi validate spec/asyncapi.yaml --fail-severity=info; \
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
$(call ensure_tool,uv)
$(call ensure_tool,action-validator)
$(call ensure_tool,actionlint)
@if [ ! -d .github/actions ]; then \
  echo "No composite actions found; skipping lint-actions"; \
else \
  find .github/actions -name 'action.yml' -print0 | xargs -0 -r uvx --from "yamllint==$(YAMLLINT_VERSION)" yamllint; \
  while IFS= read -r -d '' action; do \
    echo "$$action:"; \
    action-validator "$$action"; \
  done < <(find .github/actions -name 'action.yml' -print0); \
fi
@if [ ! -d .github/workflows ]; then \
  echo "No workflows found; skipping workflow lint"; \
else \
  find .github/workflows \( -name '*.yml' -o -name '*.yaml' \) -print0 | xargs -0 -r uvx --from "yamllint==$(YAMLLINT_VERSION)" yamllint; \
  find .github/workflows \( -name '*.yml' -o -name '*.yaml' \) -print0 | xargs -0 -r actionlint; \
fi
endef

lint-actions:
	$(LINT_ACTIONS_CMD)

PG_WORKER_PATH ?= $(CURDIR)/target/pg_worker
PG_WORKER_INSTALL_ROOT ?= $(CURDIR)/target/pg-worker-root
PG_EMBED_SETUP_UNPRIV_VERSION ?= 0.5.1
NEXTEST_TEST_THREADS ?= 1


test: test-rust test-frontend test-scripts

test-rust: workspace-sync prepare-pg-worker
	PG_EMBEDDED_WORKER=$(PG_WORKER_PATH) NEXTEST_TEST_THREADS=$(NEXTEST_TEST_THREADS) $(RUST_FLAGS_ENV) cargo nextest run --workspace --all-targets --all-features --no-fail-fast

test-frontend: deps typecheck
	pnpm run test
	pnpm run test:workspaces

# Validate the mutation-testing caller workflow contract
test-workflow-contracts:
	$(PYTHON_NO_BYTECODE_ENV) uv run --with 'pytest>=8' --with 'pyyaml>=6' pytest tests/workflow_contracts -q

# Python unit tests for the local Kubernetes preview helper
# (scripts/local_k8s). Run from the repository root so the make-target smoke
# test can resolve the real `local-k8s-*` targets, with the package exposed on
# PYTHONPATH. Test dependencies are supplied through uv's `--with`, mirroring
# the inline dependency declaration in scripts/local_k8s.py.
test-scripts:
	PYTHONPATH=scripts uv run \
		--with pytest --with pytest-mock --with hypothesis --with 'pyyaml>=6' \
		--with cyclopts==4.10.1 --with plumbum==1.9.0 \
		python -m pytest scripts/local_k8s/unittests

.ONESHELL: prepare-pg-worker
define PREPARE_PG_WORKER_CMD
set -euo pipefail
mkdir -p "$$(dirname "$(PG_WORKER_PATH)")"
if command -v pg_worker >/dev/null 2>&1; then
  install -m 0755 "$$(command -v pg_worker)" "$(PG_WORKER_PATH)"
else
  cargo install --locked --root "$(PG_WORKER_INSTALL_ROOT)" --version "$(PG_EMBED_SETUP_UNPRIV_VERSION)" --bin pg_worker pg-embed-setup-unpriv
  install -m 0755 "$(PG_WORKER_INSTALL_ROOT)/bin/pg_worker" "$(PG_WORKER_PATH)"
fi
endef

prepare-pg-worker:
	$(PREPARE_PG_WORKER_CMD)

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


audit: deps audit-node rust-audit

audit-node: deps
	pnpm -r --if-present run audit
	pnpm run audit:validate
	pnpm run audit:bun

rust-audit:
	$(call ensure_tool,cargo-audit)
	# RUSTSEC-2023-0071 is in SQLx's optional MySQL support; this workspace only enables PostgreSQL.
	# Install cargo-audit with: cargo binstall --no-confirm cargo-audit@0.22.1
	$(CARGO) audit --file Cargo.lock $(CARGO_AUDIT_IGNORES)

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

markdownlint: spelling
	@if PATH="$(BUN_PATH)" command -v markdownlint-cli2 >/dev/null 2>&1; then \
	  PATH="$(BUN_PATH)" markdownlint-cli2 '**/*.md'; \
	else \
	  PATH="$(BUN_PATH)" bun x --package=markdownlint-cli2@$(MARKDOWNLINT_CLI2_VERSION) markdownlint-cli2 '**/*.md'; \
	fi

nixie:
	$(call ensure_tool,nixie)
	$(call ensure_tool,merman-cli)
	nixie

spelling: spelling-phrase-check
	@git ls-files -z | xargs -0 -r env $(UV_ENV) \
		$(UV) tool run typos@$(TYPOS_VERSION) --config typos.toml --force-exclude --hidden

spelling-phrase-check: spelling-config
	@PYTHONPATH=scripts $(PYTHON_NO_BYTECODE_ENV) $(UV_ENV) $(UV) run --no-project --python 3.14 \
		scripts/typos_rollout_check.py --repository .

spelling-config: spelling-helper-test
	@git ls-files --error-unmatch typos.toml >/dev/null
	@$(TYPOS_CONFIG_BUILDER) --repository . --check

spelling-config-write: spelling-helper-test
	@$(TYPOS_CONFIG_BUILDER) --repository .

spelling-helper-test:
	@$(UV_ENV) $(UV) tool run ruff@$(RUFF_VERSION) format --isolated --target-version py313 --check $(SPELLING_PY_SRCS)
	@$(UV_ENV) $(UV) tool run ruff@$(RUFF_VERSION) check --isolated --target-version py313 $(SPELLING_PY_SRCS)
	@$(SPELLING_HELPER_PYTEST) $(SPELLING_PY_TESTS) -c /dev/null --rootdir=. -p no:cacheprovider $(SPELLING_COVERAGE_ARGS)

yamllint:
	$(call ensure_tool,helm)
	$(call ensure_tool,uv)
	set -o pipefail; helm template wildside ./deploy/charts/wildside --kube-version $(KUBE_VERSION) | uvx --from "yamllint==$(YAMLLINT_VERSION)" yamllint -f parsable -
