use std::process::Command;

#[test]
fn missing_target_emits_agent_readable_repair_receipt() {
    let output = Command::new(env!("CARGO_BIN_EXE_redline-testing"))
        .args([
            "run",
            "--suite",
            "sqlite_parity",
            "--target-bin",
            "/redline-testing/definitely-missing-target",
            "--sqlite-bin",
            "/redline-testing/definitely-missing-sqlite",
            "--output",
            "/tmp/redline-testing-error-receipt-contract.jsonl",
            "--progress",
            "never",
        ])
        .output()
        .expect("run redline-testing binary");

    assert!(
        !output.status.success(),
        "missing binaries must fail closed"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr is UTF-8");
    assert!(stderr.contains("error[MISSING_REFERENCE_CLI]"));
    assert!(stderr.contains("reason:"));
    assert!(stderr.contains("common fixes:"));
    assert!(stderr.contains("docs:"));
    assert!(stderr.contains("repair:"));
}
