"""The values the backend suite lanes are pinned to, and why each is.

Shared by the three contract modules that read those lanes:
`duplicate_test_lane_test.py`, `dependabot_lane_test.py` and
`suite_lane_reachability_test.py`. Split out when the first crossed the
400-line limit the Python lint gate enforces, on the seam the rest of
this suite already uses: a reader module says what the workflow
declares, this one says what it is required to declare, and the tests
say which of those requirements each case asserts.

Every value here is pinned verbatim rather than merely required to be
present. A contract that asked whether a condition mentioned an actor,
or whether a nextest filter excluded something, would be satisfied by a
narrowed condition and a widened filter alike. The comment on each entry
says what a laxer form would let through.
"""

from __future__ import annotations

#: The coverage job runs on pull requests and not for Dependabot. Pinned
#: verbatim rather than merely required to be absent, so narrowing it to
#: a push-only event or to `false` fails.
COVERAGE_JOB_CONDITION = (
    "github.actor != 'dependabot[bot]' && github.event_name != 'push'"
)

#: The feature set whose test list was measured as the superset. Written
#: as the single string the action receives, because that is what the
#: workflow declares and what a reviewer has to compare against.
MEASURED_FEATURES = "example-data metrics test-support"

#: The build job's compile-fail step, whole. Both commands are asserted
#: as the step's entire script: matching one line of a multiline script
#: is satisfied by `if false; then <command>; fi`, and matching a
#: substring is satisfied by `<command> || true`.
COMPILE_FAIL_SCRIPT = (
    "cargo test --locked --all-features -p backend --test "
    "declare_test_support_compile_fail\n"
    "cargo test --locked --features trybuild-tests -p pagination "
    "cursor_trait_bound_compile_fail_tests\n"
)

#: The guard on the build job's Dependabot test lane. It is the exact
#: inverse of the actor clause in :data:`COVERAGE_JOB_CONDITION`, and the
#: two are asserted against each other rather than each on its own: the
#: hole this repository nearly shipped was a coverage lane that skipped
#: an actor while the only other lane had been deleted.
DEPENDABOT_GUARD = "github.actor == 'dependabot[bot]'"

#: The actor clause the coverage job excludes, as it appears there.
COVERAGE_ACTOR_CLAUSE = "github.actor != 'dependabot[bot]'"

#: The nextest filter expression the Dependabot lane runs under, whole.
#: The two named binaries are compile-fail suites: they are meant to fail
#: to compile, so running them through the ordinary suite turns a passing
#: lane red. The `Compile-fail tests` step is the only thing that has
#: ever executed them, under plain `cargo test`, and it expects the
#: failure.
#:
#: Pinned as the entire expression rather than as two name lookups. A
#: contract asking only whether each name appears is satisfied by
#: `not (binary(a) | binary(b)) | binary(a)`, and by any widening that
#: leaves the names in place while changing what the filter selects.
NEXTEST_EXCLUSION = (
    "not (binary(declare_test_support_compile_fail) | binary(compile_fail_tests))"
)

#: The option nextest takes its filter expression on.
NEXTEST_FILTER_OPTION = "-E"

#: Cache paths holding embedded PostgreSQL binaries. A step restoring one
#: is preparing to start a database, so it belongs to the Dependabot lane
#: and carries that lane's guard. Both are real path entries; the
#: `#`-prefixed lines in the same block scalar are the cache action's
#: comments, not paths.
DATABASE_CACHE_PATHS = ("~/.theseus/postgresql", "~/.cache/pg-embedded/binaries")

#: The action that restores a cache, as distinct from the one that saves
#: one. The Dependabot lane needs the restore: a save-only step leaves
#: the lane downloading every PostgreSQL archive again.
CACHE_RESTORE_ACTION = "actions/cache/restore"
