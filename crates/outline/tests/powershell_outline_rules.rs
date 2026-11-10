use ast_grep_language::SupportLang;

#[allow(dead_code)]
mod common;

#[test]
fn powershell_rules_parse_and_extract_script_shapes() {
  const RULES: &str = include_str!("../src/default_rules/powershell.yml");
  common::assert_outline_snapshot(
    SupportLang::PowerShell,
    RULES,
    r#"
function Get-Foo {
  param($x)
  $x
}
filter Out-Bar { $_ }
class Point : Base {
  hidden [int]$X
  static [int]$Y = 1
  Point() {}
  [int] Sum() { return 1 }
  hidden [void] Reset() {}
}
enum Color { Red; Green = 2 }
"#,
    r#"
- Function item exported Get-Foo
- Function item exported Out-Bar
- Class item exported Point
  - Property private $X
  - Property public $Y
  - Method public Point
  - Method public Sum
  - Method private Reset
- Enum item exported Color
  - EnumMember public Red
  - EnumMember public Green
"#,
  );
}
