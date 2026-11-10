//! PowerShell support.
//!
//! PowerShell variables are spelled with `$`, so ast-grep metavariables and real
//! variables share one syntax. The contract implemented here is:
//!
//! * `$NAME` (only `A-Z`, `0-9` and `_`, not starting with a digit) is a metavariable,
//!   as in every other ast-grep language. `$$NAME`, `$$$NAME` and `$$$` work as usual.
//! * `$_` is the PowerShell pipeline variable and is always literal. Use `$_NAME`
//!   for an anonymous single-node wildcard.
//! * Every other variable (anything containing a lowercase letter, `$$`, `$?`, `$^`,
//!   `$env:Path`, `${braced}`) is literal and matched case-insensitively, as PowerShell
//!   variable names are. Write `$Home` or `$home` to match a literal `$HOME`.
//! * In fix templates, `$_`-prefixed names are literal, and a metavariable that was not
//!   captured is kept verbatim instead of being deleted.
//!
//! The grammar rejects `$$NAME` and `$$$NAME`, and no non-ASCII expando char is accepted
//! inside a variable name, so those forms are wrapped as braced variables such as
//! `${$$$ARGS}` before parsing the pattern.
use ast_grep_core::Language;
use ast_grep_core::matcher::{Pattern, PatternBuilder, PatternError};
use ast_grep_core::meta_var::{MetaVariable, extract_meta_var};
use ast_grep_core::tree_sitter::{LanguageExt, StrDoc, TSLanguage};
use std::borrow::Cow;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug)]
pub struct PowerShell;

const PIPELINE_VAR: &str = "$_";

fn variable_kinds() -> &'static [u16; 2] {
  static KINDS: OnceLock<[u16; 2]> = OnceLock::new();
  KINDS.get_or_init(|| {
    let lang = crate::parsers::language_powershell();
    [
      lang.id_for_node_kind("variable", true),
      lang.id_for_node_kind("braced_variable", true),
    ]
  })
}

fn is_meta_name_char(c: char) -> bool {
  matches!(c, 'A'..='Z' | '0'..='9' | '_')
}

fn is_word_char(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}

/// Wrap `$$NAME`, `$$$NAME` and `$$$` as `${...}` so the grammar parses them as variables.
fn pre_process_pattern(query: &str) -> Cow<'_, str> {
  if !query.contains("$$") {
    return Cow::Borrowed(query);
  }
  let chars: Vec<char> = query.chars().collect();
  let mut ret = String::with_capacity(query.len() + 8);
  let mut i = 0;
  while i < chars.len() {
    if chars[i] != '$' {
      ret.push(chars[i]);
      i += 1;
      continue;
    }
    let sigil_end = (i..chars.len())
      .find(|&j| chars[j] != '$')
      .unwrap_or(chars.len());
    let name_end = (sigil_end..chars.len())
      .find(|&j| !is_meta_name_char(chars[j]))
      .unwrap_or(chars.len());
    let sigils = sigil_end - i;
    let name = &chars[sigil_end..name_end];
    let followed_by_word = chars.get(name_end).is_some_and(|&c| is_word_char(c));
    let valid_name = name.first().is_some_and(|c| !c.is_ascii_digit());
    // `${$$$A}` is already wrapped and a backtick escapes `$` in strings
    let already_wrapped = i >= 2 && chars[i - 1] == '{' && chars[i - 2] == '$';
    let escaped = i >= 1 && chars[i - 1] == '`';
    let is_meta = match sigils {
      2 => valid_name,
      3 => valid_name || name.is_empty(),
      _ => false,
    };
    if is_meta && !followed_by_word && !already_wrapped && !escaped {
      ret.push_str("${");
      ret.extend(&chars[i..name_end]);
      ret.push('}');
    } else {
      ret.extend(&chars[i..name_end]);
    }
    i = name_end;
  }
  Cow::Owned(ret)
}

fn eq_ignore_case(a: &str, b: &str) -> bool {
  a.eq_ignore_ascii_case(b)
    || a
      .chars()
      .flat_map(char::to_lowercase)
      .eq(b.chars().flat_map(char::to_lowercase))
}

impl Language for PowerShell {
  fn pre_process_pattern<'q>(&self, query: &'q str) -> Cow<'q, str> {
    pre_process_pattern(query)
  }
  fn extract_meta_var(&self, source: &str) -> Option<MetaVariable> {
    if source == PIPELINE_VAR {
      return None;
    }
    if let Some(inner) = source.strip_prefix("${") {
      let inner = inner.strip_suffix('}')?;
      // only `$$A`, `$$$A` and `$$$` are wrapped by pre_process_pattern
      return inner
        .starts_with("$$")
        .then(|| extract_meta_var(inner, '$'))
        .flatten();
    }
    extract_meta_var(source, '$')
  }
  fn leaf_text_equivalent(&self, kind_id: u16, goal: &str, candidate: &str) -> bool {
    variable_kinds().contains(&kind_id) && eq_ignore_case(goal, candidate)
  }
  fn is_literal_template_var(&self, name: &str, rest: &str) -> bool {
    // `$_` / `$_.Name` is the pipeline variable and `$Home` is a mixed-case variable
    name.starts_with('_') || rest.starts_with(is_word_char)
  }
  fn keep_unbound_template_var(&self) -> bool {
    true
  }
  fn kind_to_id(&self, kind: &str) -> u16 {
    crate::parsers::language_powershell().id_for_node_kind(kind, true)
  }
  fn field_to_id(&self, field: &str) -> Option<u16> {
    crate::parsers::language_powershell()
      .field_id_for_name(field)
      .map(|f| f.get())
  }
  fn build_pattern(&self, builder: &PatternBuilder) -> Result<Pattern, PatternError> {
    builder.build(|src| StrDoc::try_new(src, *self))
  }
}

impl LanguageExt for PowerShell {
  fn get_ts_language(&self) -> TSLanguage {
    crate::parsers::language_powershell()
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::test::{test_match_lang, test_non_match_lang, test_replace_lang};
  use ast_grep_core::tree_sitter::LanguageExt;
  use std::collections::HashMap;

  fn test_match(query: &str, source: &str) {
    test_match_lang(query, source, PowerShell)
  }

  fn test_non_match(query: &str, source: &str) {
    test_non_match_lang(query, source, PowerShell)
  }

  fn test_replace(src: &str, pattern: &str, replacer: &str) -> String {
    test_replace_lang(src, pattern, replacer, PowerShell)
  }

  fn captures(query: &str, source: &str) -> HashMap<String, String> {
    let root = PowerShell.ast_grep(source);
    let found = root
      .root()
      .find(query)
      .unwrap_or_else(|| panic!("{query} should match {source}"));
    HashMap::from(found.get_env().clone())
  }

  fn assert_no_error(source: &str) {
    let root = PowerShell.ast_grep(source);
    let sexp = root.root().get_inner_node().to_sexp();
    assert!(
      !sexp.contains("ERROR") && !sexp.contains("MISSING"),
      "{source} => {sexp}"
    );
  }

  #[test]
  fn test_powershell_pre_process() {
    let pp = |s| PowerShell.pre_process_pattern(s).to_string();
    assert_eq!(pp("Get-Item $A"), "Get-Item $A");
    assert_eq!(pp("Get-Item $$$ARGS"), "Get-Item ${$$$ARGS}");
    assert_eq!(pp("Foo $$$"), "Foo ${$$$}");
    assert_eq!(pp("Foo $$A"), "Foo ${$$A}");
    // literal PowerShell syntax is untouched
    assert_eq!(pp("$$; $?; $_; $home"), "$$; $?; $_; $home");
    assert_eq!(pp("$$$abc"), "$$$abc");
    assert_eq!(pp("$$Ab"), "$$Ab");
    assert_eq!(pp("\"`$$$A\""), "\"`$$$A\"");
    assert_eq!(pp("${$$$A}"), "${$$$A}");
  }

  #[test]
  fn test_powershell_extract_meta_var() {
    use MetaVariable::*;
    let ex = |s| PowerShell.extract_meta_var(s);
    assert_eq!(ex("$A"), Some(Capture("A".into(), true)));
    assert_eq!(ex("$_A"), Some(Dropped(true)));
    assert_eq!(ex("${$$A}"), Some(Capture("A".into(), false)));
    assert_eq!(ex("${$$$ARGS}"), Some(MultiCapture("ARGS".into())));
    assert_eq!(ex("${$$$}"), Some(Multiple));
    for literal in [
      "$_",
      "$a",
      "$Home",
      "$$",
      "$?",
      "$env:Path",
      "${A}",
      "${$A}",
    ] {
      assert_eq!(ex(literal), None, "{literal}");
    }
  }

  #[test]
  fn test_powershell_grammar_corpus() {
    for src in [
      "Get-ChildItem -Path . -Recurse | Where-Object { $_.Length -gt 1kb } | Select-Object Name",
      "function Get-Foo { param([Parameter(Mandatory)][string]$Name, [int]$Count = 1)
  $Name * $Count
}",
      "class Point { [int]$X; [int]$Y; Point([int]$x, [int]$y) { $this.X = $x; $this.Y = $y } [int] Sum() { return $this.X + $this.Y } }",
      "$params = @{ Path = 'C:\\temp'; Recurse = $true }; Get-ChildItem @params",
      "$list = @(1, 2, 3); $list | ForEach-Object { $PSItem * 2 }",
      "$sb = { param($x) $x + 1 }; & $sb 1",
      "Write-Host \"Hello $env:USERNAME, pid $PID and ${braced name}\"",
      "$here = @\"\nvalue $x\n\"@",
      "$lit = @'\nvalue $x\n'@",
      "<# block\ncomment #>\n# line comment\n$a = 1",
      "try { throw 'x' } catch [System.Exception] { $_.Exception.Message } finally { }",
      "switch ($x) {\n  1 { 'one' }\n  default { 'other' }\n}",
      "$Global:Var = $Script:Other",
      "@{ ModuleVersion = '1.0.0'; RootModule = 'Mod.psm1'; FunctionsToExport = @('Get-Foo') }",
    ] {
      assert_no_error(src);
    }
  }

  #[test]
  fn test_powershell_documented_grammar_limitations() {
    // tree-sitter-powershell 0.26.6 limitations, see POWERSHELL_SUPPORT_PLAN.md
    for src in ["$µA = 1", "[µT]$x", "$ÀB", "ForEach-Object { $_ }"] {
      let root = PowerShell.ast_grep(src);
      assert!(root.root().get_inner_node().has_error(), "{src}");
    }
  }

  #[test]
  fn test_powershell_pattern() {
    test_match("Get-Item $A", "Get-Item $path");
    test_match("$A = $B", "$x = 1");
    test_match("$A + $B", "$a + 2");
    test_match("$A.Name", "$item.Name");
    test_match("$A -eq $B", "$x -eq $null");
    test_match(
      "if ($A) { $$$BODY }",
      "if ($ok) { Write-Host 1; Write-Host 2 }",
    );
    test_match("$A.Invoke($$$ARGS)", "$o.Invoke(1, 2, 3)");
    test_match("function $F { $$$BODY }", "function Get-Foo { 'foo' }");
    test_match("[int]$A", "[int]$count");
    test_non_match("Get-Item $A", "Set-Item $path");
  }

  #[test]
  fn test_powershell_command_arguments() {
    let root = PowerShell.ast_grep("Get-Item -Path $p -Force");
    let found = root.root().find("Get-Item $$$ARGS").expect("should match");
    let args = found.get_env().get_multiple_matches("ARGS");
    assert_eq!(args.first().map(|n| n.text()).as_deref(), Some("-Path"));
    assert_eq!(args.last().map(|n| n.text()).as_deref(), Some("-Force"));
    let env = captures("Get-Item -Path $P", "Get-Item -Path $file");
    assert_eq!(env["P"], "$file");
    test_match("Get-Item $$$", "Get-Item a b c");
  }

  #[test]
  fn test_powershell_literal_variables() {
    // `$_` is the pipeline variable, never a wildcard
    test_match("$_.Name", "$x | % { $_.Name }");
    test_non_match("$_.Name", "$x.Name");
    // `$_NAME` is the anonymous wildcard
    test_match("$_ANY.Name", "$x.Name");
    test_match("$$", "Write-Host $$");
    test_match("$?", "if ($?) { 1 }");
    test_match("$env:Path", "$env:Path += ';C:\\bin'");
    test_non_match("$env:Path", "$env:Temp");
    test_non_match("$home", "$profile");
  }

  #[test]
  fn test_powershell_case_insensitive_variables() {
    test_match("$Home", "$HOME");
    test_match("$home", "$HOME");
    test_match("$psitem.Name", "$PSItem.Name");
    test_match("$Env:path", "$env:PATH");
    test_match("${my var}", "${My Var}");
    // the case rule only applies to variables
    test_non_match("'abc'", "'ABC'");
    test_non_match("$x.name", "$x.NAME");
    // all-caps is a capture, so it matches anything
    test_match("$HOME", "$profile");
  }

  #[test]
  fn test_powershell_literal_and_capture_together() {
    let env = captures("Join-Path $Home $CHILD", "Join-Path $HOME 'docs'");
    assert_eq!(env["CHILD"], "'docs'");
    test_non_match("Join-Path $Home $CHILD", "Join-Path $PWD 'docs'");
  }

  #[test]
  fn test_powershell_repeated_capture() {
    test_match("$A = $A + 1", "$i = $i + 1");
    // variables are case-insensitive, so they are the same variable
    test_match("$A = $A + 1", "$i = $I + 1");
    test_non_match("$A = $A + 1", "$i = $j + 1");
  }

  #[test]
  fn test_powershell_replace() {
    let ret = test_replace("$x = 1", "$A = $B", "$A = $B + 1");
    assert_eq!(ret, "$x = 1 + 1");
    let ret = test_replace(
      "Write-Host $name",
      "Write-Host $A",
      "Write-Output \"$A at $Home\"",
    );
    assert_eq!(ret, "Write-Output \"$name at $Home\"");
    let ret = test_replace(
      "Get-Item -Path $p -Force",
      "Get-Item $$$ARGS",
      "Get-ChildItem $$$ARGS",
    );
    assert_eq!(ret, "Get-ChildItem -Path $p -Force");
  }

  #[test]
  fn test_powershell_replace_keeps_powershell_variables() {
    // `$_` in a fix is the pipeline variable
    let ret = test_replace(
      "$a | % { $_.Name }",
      "% { $$$BODY }",
      "ForEach-Object { $_.FullName }",
    );
    assert_eq!(ret, "$a | ForEach-Object { $_.FullName }");
    // an uncaptured all-caps name is kept rather than deleted
    let ret = test_replace("Get-Item $x", "Get-Item $A", "Join-Path $HOME $A");
    assert_eq!(ret, "Join-Path $HOME $x");
    let ret = test_replace("Write-Host $x", "Write-Host $A", "Write-Host $$ $? $A");
    assert_eq!(ret, "Write-Host $$ $? $x");
  }

  #[test]
  fn test_powershell_crlf_and_non_ascii() {
    let ret = test_replace(
      "$a = 1\r\nWrite-Host 'héllo 😀'\r\n$b = 2",
      "Write-Host $A",
      "Write-Output $A",
    );
    assert_eq!(ret, "$a = 1\r\nWrite-Output 'héllo 😀'\r\n$b = 2");
  }
}
