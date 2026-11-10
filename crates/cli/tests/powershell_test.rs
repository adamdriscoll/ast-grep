mod common;

use anyhow::Result;
use assert_cmd::{Command, cargo_bin};
use common::create_test_files;
use predicates::prelude::*;
use predicates::str::contains;

const SCRIPT: &str = "Get-ChildItem $HOME | Where-Object { $_.Length -gt 1kb }\nWrite-Host $name\n";

#[test]
fn test_powershell_infer_extensions() -> Result<()> {
  let dir = create_test_files([
    ("a.ps1", "Write-Host 'script'"),
    ("b.psm1", "Write-Host 'module'"),
    ("c.psd1", "@{ ModuleVersion = '1.0.0' }"),
    ("d.txt", "Write-Host 'text'"),
  ])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["-p", "Write-Host $A"])
    .assert()
    .success()
    .stdout(contains("a.ps1"))
    .stdout(contains("b.psm1"))
    .stdout(contains("d.txt").not());
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["-p", "'1.0.0'"])
    .assert()
    .success()
    .stdout(contains("c.psd1"));
  Ok(())
}

#[test]
fn test_powershell_json_and_language_globs() -> Result<()> {
  let dir = create_test_files([
    (
      "sgconfig.yml",
      "ruleDirs: []\nlanguageGlobs:\n  powershell: ['*.pwsh']\n",
    ),
    ("a.pwsh", "Write-Host $name"),
  ])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["run", "-p", "Write-Host $A", "--json=compact"])
    .assert()
    .success()
    .stdout(contains("\"language\":\"PowerShell\""))
    .stdout(contains("\"text\":\"$name\""));
  Ok(())
}

#[test]
fn test_powershell_lang_aliases() -> Result<()> {
  for alias in ["powershell", "pwsh", "ps1", "PowerShell"] {
    Command::new(cargo_bin!())
      .args(["-p", "Write-Host $A", "-l", alias, "--stdin"])
      .write_stdin(SCRIPT)
      .assert()
      .success()
      .stdout(contains("Write-Host $name"));
  }
  Ok(())
}

#[test]
fn test_powershell_literal_variables() -> Result<()> {
  // `$_` is literal, `$Home` matches `$HOME` case-insensitively
  Command::new(cargo_bin!())
    .args(["-p", "$_.Length", "-l", "ps1", "--stdin"])
    .write_stdin(SCRIPT)
    .assert()
    .success()
    .stdout(contains("$_.Length"));
  Command::new(cargo_bin!())
    .args(["-p", "Get-ChildItem $Home", "-l", "ps1", "--stdin"])
    .write_stdin(SCRIPT)
    .assert()
    .success()
    .stdout(contains("Get-ChildItem $HOME"));
  Command::new(cargo_bin!())
    .args(["-p", "Get-ChildItem $profile", "-l", "ps1", "--stdin"])
    .write_stdin(SCRIPT)
    .assert()
    .failure();
  Ok(())
}

#[test]
fn test_powershell_rewrite() -> Result<()> {
  let dir = create_test_files([("a.ps1", SCRIPT)])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args([
      "-p",
      "Write-Host $A",
      "-r",
      "Write-Output \"$A in $HOME\" $$$REST",
      "-U",
    ])
    .assert()
    .success();
  let updated = std::fs::read_to_string(dir.path().join("a.ps1"))?;
  assert_eq!(
    updated,
    "Get-ChildItem $HOME | Where-Object { $_.Length -gt 1kb }\nWrite-Output \"$name in $HOME\" $$$REST\n"
  );
  Ok(())
}

const RULE: &str = r#"
id: no-write-host
message: Avoid Write-Host $A
severity: warning
language: powershell
rule:
  pattern: Write-Host $A
constraints:
  A:
    regex: '^\$'
transform:
  UPPER:
    convert:
      source: $A
      toCase: upperCase
fix: Write-Output $UPPER $Home $_
"#;

#[test]
fn test_powershell_scan_and_fix() -> Result<()> {
  let dir = create_test_files([
    ("rule.yml", RULE),
    ("a.ps1", "Write-Host $name\nWrite-Host 'literal'\n"),
  ])?;
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "-r", "rule.yml"])
    .assert()
    .success()
    .stdout(contains("no-write-host"))
    .stdout(contains("Avoid Write-Host $name"))
    .stdout(contains("Avoid Write-Host 'literal'").not());
  Command::new(cargo_bin!())
    .current_dir(dir.path())
    .args(["scan", "-r", "rule.yml", "-U"])
    .assert()
    .success();
  let updated = std::fs::read_to_string(dir.path().join("a.ps1"))?;
  assert_eq!(
    updated,
    "Write-Output $NAME $Home $_\nWrite-Host 'literal'\n"
  );
  Ok(())
}

#[test]
fn test_powershell_fix_with_uncaptured_all_caps_var_is_rejected() -> Result<()> {
  let rule =
    "{id: t, language: powershell, rule: {pattern: Write-Host $A}, fix: Join-Path $HOME $A}";
  Command::new(cargo_bin!())
    .args(["scan", "--stdin", "--inline-rules", rule])
    .write_stdin("Write-Host $x")
    .assert()
    .failure()
    .stderr(contains("HOME"));
  Ok(())
}
