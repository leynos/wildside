"""The values CV-005 pins for this repository, and why each is pinned.

Shared by `codescene_coverage_baseline_test.py`, which holds the three
absences every pull-request workflow must satisfy, and
`codescene_publisher_test.py`, which holds what the one publishing
workflow must do. Split out when the first crossed the 400-line limit
the Python lint gate enforces, on the seam the rest of this suite uses:
a reader says what a thing is, this says what it must be, and the tests
say which requirement each case asserts.

Every value here is pinned verbatim rather than merely required to be
present, and the comment on each says what a laxer form would let
through. Three of them exist because the contract was once satisfiable
without them.
"""

from __future__ import annotations

#: The *name* of the variable the CodeScene CLI authenticates with. This
#: holds the identifier, never a value: the contract searches for the
#: name because both the `env:` key and the `secrets.` reference spell it
#: identically, and either one puts the credential on the lane.
CODESCENE_CREDENTIAL_NAME = "CS_ACCESS_TOKEN"

#: Any action under a path naming CodeScene. Matched on the path rather
#: than on a pin, so a repin does not silently reintroduce the step.
CODESCENE_ACTION_MARKER = "codescene"

#: The service's host. Matched case-insensitively, because DNS names are,
#: and kept apart from the credential and action checks: a step can reach
#: the project API by curling it, naming neither the action, the CLI nor
#: the credential.
CODESCENE_HOST = "codescene.io"

#: The CLI, in both spellings: the standalone `cs-coverage` binary the
#: shared action installs, and the `cs` binary's `coverage` subcommand.
COVERAGE_CLI = "cs-coverage"
CLI_BINARY = "cs"
CLI_SUBCOMMAND = "coverage"

#: The workflow allowed to talk to CodeScene, and the event it does it on.
PUBLISHER = "coverage-main.yml"
PUBLISHER_EVENT = "push"

#: The only branch whose coverage may advance the ratchet baseline. A
#: push trigger without this would let a feature branch publish, and the
#: baseline every pull request is then measured against would no longer
#: be the trunk's.
PUBLISHER_BRANCHES = ["main"]

#: The `if` the publisher's upload step must carry, whole. The token
#: clause alone is not enough, and the ref clause is not redundant with
#: the push branch filter: `workflow_dispatch` is mandatory on this
#: workflow and a dispatch runs from whichever branch it was started on.
PUBLISHER_UPLOAD_CONDITION = (
    "env.CS_ACCESS_TOKEN != '' && github.ref == 'refs/heads/main'"
)

#: The concurrency group the publisher serializes on, and the setting
#: that must not be true. Cancelling a publisher abandons a baseline
#: write half done, which is the failure the group exists to prevent.
PUBLISHER_CONCURRENCY_GROUP = "coverage-main-${{ github.ref }}"

#: The upload mode, and the value the action assumes when the input is
#: omitted. `coverage-main.yml` omits it, so a contract reading the input
#: alone would find nothing; the effective mode is what publishes.
#: Confirmed against `upload-codescene-coverage`'s `action.yml` at the
#: pinned c5a54701, where `mode` is `required: false` with
#: `default: upload`.
UPLOAD_MODE = "upload"

#: The action that produces coverage on both lanes.
GENERATE_COVERAGE = "generate-coverage"

#: The inputs that decide which coverage number is produced. The ratchet
#: compares a pull request against a baseline the publisher wrote, so a
#: lane disagreeing with the publisher on any of these is comparing two
#: different measurements and the comparison means nothing.
BASELINE_INPUTS = ("language", "format", "output-path", "use-cargo-nextest", "features")

#: Every workflow that runs on a contributor's pull request, pinned. The
#: three absence contracts below are parametrized over a discovered list,
#: and a discovery that quietly returned fewer workflows would leave them
#: passing while checking less. Pinning the set is what makes the count
#: visible in a diff.
EXPECTED_PULL_REQUEST_WORKFLOWS = ("ci.yml", "dependabot-automerge.yml")
