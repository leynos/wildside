SHELL := bash
KUBE_VERSION ?= 1.31.0
# Supported DigitalOcean Kubernetes release. Update to a current patch from
# the 1.33.x, 1.32.x or 1.31.x series as listed in the DigitalOcean docs.
# Latest tested patch: https://docs.digitalocean.com/products/kubernetes/releases/
DOKS_KUBERNETES_VERSION ?= 1.33.1-do.3

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
ORVAL_VERSION ?= 7.11.2
BIOME_VERSION ?= 2.3.1
TSC_VERSION ?= 5.9.2
MARKDOWNLINT_CLI2_VERSION ?= 0.14.0
OPENAPI_SPEC ?= spec/openapi.json

# Place one consolidated PHONY declaration near the top of the file
.PHONY: all clean be fe fe-build openapi gen docker-up docker-down fmt lint test typecheck deps lockfile \
        check-fmt check-test-deps markdownlint markdownlint-docs mermaid-lint nixie yamllint audit \
        lint-asyncapi lint-openapi lint-makefile lint-actions lint-infra conftest tofu doks-test doks-policy fluxcd-test fluxcd-policy \
        vault-appliance-test vault-appliance-policy dev-cluster-test workspace-sync scripts-test

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
	$(call exec_or_bunx,biome,ci --formatter-enabled=true --reporter=github frontend-pwa packages,@biomejs/biome@$(BIOME_VERSION))
	$(MAKE) lint-asyncapi lint-openapi lint-makefile lint-actions lint-infra

# Lint AsyncAPI spec if present. Split to keep `lint` target concise per checkmake rules.
lint-asyncapi:
	if [ -f spec/asyncapi.yaml ]; then \
	  bun x --package=@asyncapi/cli@$(ASYNCAPI_CLI_VERSION) asyncapi validate spec/asyncapi.yaml; \
	fi

# Lint OpenAPI spec with Redocly CLI
lint-openapi:
	$(call ensure_tool,python3)
	@if ! grep -F -q "$(OPENAPI_SPEC):" .redocly.lint-ignore.yaml; then \
		echo "OpenAPI ignore file missing entry for $(OPENAPI_SPEC)" >&2; \
		exit 1; \
	fi
	@python3 scripts/check_redoc_ignore.py
	$(call exec_or_bunx,redocly,lint $(OPENAPI_SPEC),@redocly/cli@$(REDOCLY_CLI_VERSION))

# Validate Makefile style and structure
lint-makefile:
	command -v checkmake >/dev/null || { echo "checkmake is not installed" >&2; exit 1; }
	command -v mbake >/dev/null || { echo "mbake is not installed" >&2; exit 1; }
	checkmake Makefile
	mbake validate Makefile

lint-actions:
	$(call ensure_tool,yamllint)
	$(call ensure_tool,action-validator)
	@actions=$$(if [ -d .github/actions ]; then find .github/actions -name 'action.yml' -print; fi); \
	if [ -z "$$actions" ]; then \
	  echo "No composite actions found; skipping lint-actions"; \
	else \
	  echo "$$actions" | xargs yamllint; \
	  while IFS= read -r action; do \
	    echo "$$action:"; \
	    action-validator "$$action"; \
	  done <<< "$$actions"; \
	fi

lint-infra:
	$(call ensure_tool,tflint)
	$(call ensure_tool,uvx)
	cd infra/modules/doks && tflint --init && tflint --config .tflint.hcl
	cd infra/clusters/dev && tflint --init && tflint --config .tflint.hcl
	cd infra/modules/fluxcd && tflint --init && tflint --config .tflint.hcl
	cd infra/modules/vault_appliance && tflint --init && tflint --config .tflint.hcl
	mkdir -p .uv-cache
	UV_CACHE_DIR=$(CURDIR)/.uv-cache uvx checkov -d infra

test: workspace-sync deps typecheck prepare-pg-worker
	PG_EMBEDDED_WORKER=/var/tmp/pg_worker $(RUST_FLAGS_ENV) cargo nextest run --workspace --all-targets --all-features
	pnpm -r --if-present --silent run test
	$(MAKE) scripts-test

.PHONY: prepare-pg-worker
prepare-pg-worker:
	$(RUST_FLAGS_ENV) cargo build -p backend --bin pg_worker
	install -m 0755 target/debug/pg_worker /var/tmp/pg_worker
	find /var/tmp -maxdepth 1 -type d -name 'pg-embed-*' -exec rm -rf {} +

scripts-test:
	$(call ensure_tool,uv)
	uv run \
		--with pytest \
		--with plumbum \
		--with cyclopts \
		--with pyyaml \
		--with "cmd-mox@git+https://github.com/leynos/cmd-mox@28acd288975f15e4c360d62e431950820dbcb27a" \
		pytest scripts/tests

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

check-fmt:
	@if cargo fmt --help | grep -q -- '--workspace'; then \
		cargo fmt --workspace --all -- --check; \
	else \
		cargo fmt --all -- --check; \
	fi
	$(call exec_or_bunx,biome,ci --formatter-enabled=true --reporter=github frontend-pwa packages,@biomejs/biome@$(BIOME_VERSION))

INFRA_TEST_TARGETS := \
        doks-test \
        doks-policy \
        dev-cluster-test \
        fluxcd-test \
        fluxcd-policy \
        vault-appliance-test \
        vault-appliance-policy

$(INFRA_TEST_TARGETS): check-test-deps

check-test-deps:
	./scripts/check_test_dependencies.py

markdownlint:
	$(call exec_or_bunx,markdownlint-cli2,'**/*.md',markdownlint-cli2@$(MARKDOWNLINT_CLI2_VERSION))

nixie:
	node scripts/install-mermaid-browser.mjs
	# CI currently requires --no-sandbox; remove once nixie supports
	# environment variable control for this option
	nixie --no-sandbox

yamllint:
	command -v helm >/dev/null && command -v yamllint >/dev/null && command -v yq >/dev/null
	set -o pipefail; helm template wildside ./deploy/charts/wildside --kube-version $(KUBE_VERSION) | yamllint -f parsable -
	[ ! -f deploy/k8s/overlays/production/patch-helmrelease-values.yaml ] || \
	(set -o pipefail; helm template wildside ./deploy/charts/wildside -f <(yq e '.spec.values' deploy/k8s/overlays/production/patch-helmrelease-values.yaml) --kube-version $(KUBE_VERSION) | yamllint -f parsable -)

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
	cd infra/modules/doks/tests && DOKS_KUBERNETES_VERSION=$(DOKS_KUBERNETES_VERSION) go test -v
	# Optional: surface "changes pending" in logs without failing CI
	tofu -chdir=infra/modules/doks/examples/basic plan -detailed-exitcode \
	-var cluster_name=test \
	-var region=nyc1 \
	-var kubernetes_version=$(DOKS_KUBERNETES_VERSION) \
	-var 'node_pools=[{"name"="default","size"="s-2vcpu-2gb","node_count"=2,"auto_scale"=false,"min_nodes"=2,"max_nodes"=2}]' \
	|| test $$? -eq 2
	$(MAKE) doks-policy

doks-policy: conftest tofu
	tofu -chdir=infra/modules/doks/examples/basic plan -out=tfplan.binary -detailed-exitcode \
	-var cluster_name=test \
	-var region=nyc1 \
	-var kubernetes_version=$(DOKS_KUBERNETES_VERSION) \
	-var 'node_pools=[{"name"="default","size"="s-2vcpu-2gb","node_count"=2,"auto_scale"=false,"min_nodes"=2,"max_nodes"=2}]' \
	|| test $$? -eq 2
	tofu -chdir=infra/modules/doks/examples/basic show -json tfplan.binary > infra/modules/doks/examples/basic/plan.json
	conftest test infra/modules/doks/examples/basic/plan.json --policy infra/modules/doks/policy

dev-cluster-test: conftest tofu
	DOKS_KUBERNETES_VERSION=$(DOKS_KUBERNETES_VERSION) ./scripts/dev-cluster-test.sh

fluxcd-test:
	tofu fmt -check infra/modules/fluxcd
	tofu -chdir=infra/modules/fluxcd/examples/basic init
	if [ -n "$(FLUX_KUBECONFIG_PATH)" ]; then \
		TF_IN_AUTOMATION=1 tofu -chdir=infra/modules/fluxcd/examples/basic validate -no-color \
			-var "kubeconfig_path=$(FLUX_KUBECONFIG_PATH)"; \
	else \
		echo "Skipping fluxcd validate; set FLUX_KUBECONFIG_PATH to enable"; \
	fi
	command -v tflint >/dev/null
	cd infra/modules/fluxcd && tflint --init && tflint --config .tflint.hcl --version && tflint --config .tflint.hcl
	cd infra/modules/fluxcd/tests && KUBECONFIG="$(FLUX_KUBECONFIG_PATH)" go test -v
	if [ -n "$(FLUX_KUBECONFIG_PATH)" ]; then \
		TF_IN_AUTOMATION=1 tofu -chdir=infra/modules/fluxcd/examples/basic plan -input=false -no-color -detailed-exitcode \
			-var "git_repository_url=${FLUX_GIT_REPOSITORY_URL:-https://github.com/fluxcd/flux2-kustomize-helm-example.git}" \
			-var "git_repository_path=${FLUX_GIT_REPOSITORY_PATH:-./clusters/my-cluster}" \
			-var "git_repository_branch=${FLUX_GIT_REPOSITORY_BRANCH:-main}" \
			-var "kubeconfig_path=$(FLUX_KUBECONFIG_PATH)"; \
		status=$$?; \
		if [ $$status -ne 0 ] && [ $$status -ne 2 ]; then exit $$status; fi; \
	else \
		echo "Skipping fluxcd plan -detailed-exitcode; set FLUX_KUBECONFIG_PATH to enable"; \
	fi
	$(MAKE) fluxcd-policy

# Delegate the Terraform plan and Conftest execution to a script so the target
# stays readable while still supporting temporary files and clean shutdown.
fluxcd-policy: conftest tofu
	if [ -z "$(FLUX_KUBECONFIG_PATH)" ]; then \
	echo "Skipping fluxcd-policy; set FLUX_KUBECONFIG_PATH to run"; \
	else \
	env \
	FLUX_KUBECONFIG_PATH="$(FLUX_KUBECONFIG_PATH)" \
	FLUX_GIT_REPOSITORY_URL="$(FLUX_GIT_REPOSITORY_URL)" \
	FLUX_GIT_REPOSITORY_PATH="$(FLUX_GIT_REPOSITORY_PATH)" \
	FLUX_GIT_REPOSITORY_BRANCH="$(FLUX_GIT_REPOSITORY_BRANCH)" \
	FLUX_POLICY_PARAMS_JSON="$(FLUX_POLICY_PARAMS_JSON)" \
	FLUX_POLICY_DATA="$(FLUX_POLICY_DATA)" \
	./scripts/fluxcd-policy.sh; \
	fi

vault-appliance-test:
	tofu fmt -check infra/modules/vault_appliance
	tofu -chdir=infra/modules/vault_appliance/examples/basic init
	tofu -chdir=infra/modules/vault_appliance/examples/basic validate
	command -v tflint >/dev/null
	cd infra/modules/vault_appliance && tflint --init && tflint --config .tflint.hcl --version && tflint --config .tflint.hcl
	conftest test infra/modules/vault_appliance --policy infra/modules/vault_appliance/policy --ignore ".terraform"
	cd infra/modules/vault_appliance/tests && go test -v
	DIGITALOCEAN_TOKEN=dummy tofu -chdir=infra/modules/vault_appliance/examples/basic plan -detailed-exitcode \
	-var name=vault-ci \
	-var region=nyc1 \
	-var 'allowed_ssh_cidrs=["203.0.113.10/32"]' \
	-var certificate_common_name=vault-ci.example.test \
	-var 'certificate_dns_names=["vault-ci.example.test"]' \
	-var recovery_shares=5 \
	-var recovery_threshold=3 \
	|| test $$? -eq 2
	$(MAKE) vault-appliance-policy

vault-appliance-policy: conftest tofu
	DIGITALOCEAN_TOKEN=dummy tofu -chdir=infra/modules/vault_appliance/examples/basic plan -out=tfplan.binary -detailed-exitcode \
	-var name=vault-ci \
	-var region=nyc1 \
	-var 'allowed_ssh_cidrs=["203.0.113.10/32"]' \
	-var certificate_common_name=vault-ci.example.test \
	-var 'certificate_dns_names=["vault-ci.example.test"]' \
	-var recovery_shares=5 \
	-var recovery_threshold=3 \
	|| test $$? -eq 2
	DIGITALOCEAN_TOKEN=dummy tofu -chdir=infra/modules/vault_appliance/examples/basic show -json tfplan.binary > infra/modules/vault_appliance/examples/basic/plan.json
	conftest test infra/modules/vault_appliance/examples/basic/plan.json --policy infra/modules/vault_appliance/policy
	rm -f infra/modules/vault_appliance/examples/basic/tfplan.binary infra/modules/vault_appliance/examples/basic/plan.json
