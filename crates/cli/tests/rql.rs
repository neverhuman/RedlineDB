use assert_cmd::Command;

#[test]
fn cli_rql_json_uses_existing_render_modes() {
    let input = r#"
{
  "statements": [
    {
      "type": "create_table",
      "table": {"name": "items"},
      "columns": [
        {"name": "id", "declared_type": "INTEGER", "primary_key": true},
        {"name": "label", "declared_type": "TEXT"}
      ]
    },
    {
      "type": "insert",
      "table": {"name": "items"},
      "columns": ["id", "label"],
      "values": [
        [
          {"type": "integer", "value": 1},
          {"type": "text", "value": "Ada"}
        ]
      ]
    },
    {
      "type": "select",
      "projection": [
        {
          "type": "expr",
          "expr": {
            "type": "column",
            "column": {"name": "label"}
          }
        }
      ],
      "from": {"name": {"name": "items"}},
      "filter": {
        "type": "binary",
        "op": "eq",
        "left": {
          "type": "column",
          "column": {"name": "id"}
        },
        "right": {"type": "integer", "value": 1}
      }
    }
  ]
}
"#;

    let mut cmd = Command::cargo_bin("redlinedb").expect("binary");
    cmd.arg("--rql")
        .arg(":memory:")
        .write_stdin(input)
        .assert()
        .success()
        .stdout("Ada\n");
}
