module wildside/infra/modules/cnpg/tests

go 1.22

require (
	github.com/gruntwork-io/terratest v0.47.2
	github.com/stretchr/testify v1.9.0
	wildside/infra/testutil v0.0.0
)

replace wildside/infra/testutil => ../../../testutil
