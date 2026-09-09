use std::{fs, path::Path, process::Command};

#[test]
fn score_policy_cli_rejects_regressions_with_golden_diagnostics() {
    let directory = tempfile::tempdir().unwrap();
    let before = directory.path().join("before.json");
    let after = directory.path().join("after.json");
    fs::write(
        &before,
        r#"{"score":99,"raw_score":100,"hard_findings":0,"soft_findings":1,"caps_applied":[]}"#,
    )
    .unwrap();
    fs::write(
        &after,
        r#"{"score":98,"raw_score":100,"hard_findings":1,"soft_findings":1,"caps_applied":["cap-a"]}"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_score_policy"))
        .args(["compare"])
        .arg(&before)
        .arg(&after)
        .arg("push")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        concat!(
            "ERROR: score ratchet rejected this push:\n",
            "  - score decreased: 99 -> 98\n",
            "  - hard_findings increased: 0 -> 1\n",
            "  - finding_count increased: 1 -> 2\n",
            "  - applied cap count increased: 0 -> 1\n",
            "  - new applied caps: cap-a\n"
        )
    );
}

#[test]
fn perf_evidence_cli_emits_frozen_statistics_golden() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/perf-evidence/measured.jsonl");
    let output = Command::new(env!("CARGO_BIN_EXE_perf_evidence"))
        .arg("summarize-jsonl")
        .arg(fixture)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        concat!(
            "  cases measured: 10\n",
            "  samples:        10\n",
            "  ratio median:   5.500\n",
            "  ratio p90:      9.900\n",
            "  cases faster than sqlite: 0/10\n"
        )
    );
}
