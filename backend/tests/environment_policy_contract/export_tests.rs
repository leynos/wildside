//! Tests for detecting global Makefile environment exports.

use super::makefile_globally_exports;

#[test]
fn makefile_global_export_detection_handles_bare_and_named_exports() {
    assert!(makefile_globally_exports("export\n", "PG_PASSWORD"));
    assert!(makefile_globally_exports(
        "export # export all Make variables\n",
        "PG_PASSWORD"
    ));
    assert!(makefile_globally_exports(
        "export PG_PASSWORD=value\n",
        "PG_PASSWORD"
    ));
    assert!(makefile_globally_exports(
        "export PG_PASSWORD OTHER\n",
        "PG_PASSWORD"
    ));
    assert!(makefile_globally_exports(
        "export OTHER PG_PASSWORD\n",
        "PG_PASSWORD"
    ));
    assert!(!makefile_globally_exports(
        "export PG_PASSWORD_EXTRA=value\n",
        "PG_PASSWORD"
    ));
    assert!(!makefile_globally_exports(
        "exported PG_PASSWORD=value\n",
        "PG_PASSWORD"
    ));
    assert!(!makefile_globally_exports(
        "\texport PG_PASSWORD=value\n",
        "PG_PASSWORD"
    ));
    for name in ["PG_PASSWORD", "POSTGRESQL_RELEASES_URL"] {
        for operator in ["=", ":=", "::=", ":::=", "?=", "+=", "!="] {
            let directive = format!("export {name}{operator}secret\n");
            assert!(
                makefile_globally_exports(&directive, name),
                "the export contract must detect `{directive}`"
            );
            let longer_name = format!("export {name}_EXTRA{operator}secret\n");
            assert!(
                !makefile_globally_exports(&longer_name, name),
                "the export contract must not match `{longer_name}` as `{name}`"
            );
        }
    }
}
