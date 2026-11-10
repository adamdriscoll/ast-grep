# Plan: PowerShell support in ast-grep

## Implementation status

Built-in PowerShell support is implemented for the Rust CLI, YAML rules, LSP,
outline, schemas and Python bindings, using
[`tree-sitter-powershell` 0.26.6](https://docs.rs/crate/tree-sitter-powershell/0.26.6)
(MIT, ABI 15) as an optional `builtin-parser` dependency.

- **Names:** `powershell`, `pwsh`, `ps1` (and `PowerShell`); files `*.ps1`,
  `*.psm1`, `*.psd1`. Other extensions can be mapped with `languageGlobs`.
  A built-in name wins over a custom language with the same name, while a
  custom language's file extensions still win for path inference, matching
  every other built-in language.
- **Pattern contract (decided):**
  - `$NAME` with only `A-Z`, `0-9`, `_` is a metavariable as in every other
    language; `$$NAME`, `$$$NAME` and `$$$` work as usual (they are rewritten
    to `${$$$NAME}` before parsing because the grammar rejects them).
  - `$_` is always the literal pipeline variable. `$_NAME` remains the
    anonymous wildcard.
  - Every other variable is literal: anything with a lowercase letter
    (`$Home`, `$args`, `$PSItem`), `$$`, `$?`, `$env:Path`, `${braced}`.
    Variables are matched **case-insensitively**, like PowerShell itself, so
    write `$Home` to match a literal `$HOME`. Repeated captures (`$A = $A + 1`)
    also compare variables case-insensitively. Non-variable text (strings,
    member names, commands) stays case-sensitive.
- **Fix contract:** `$_` and `$_.Name` are literal; a `$` name followed by a
  lowercase letter (`$Home`) is literal. In YAML rules an uncaptured
  all-caps name such as `$HOME` is rejected as an undefined metavariable
  (write `$Home`); in CLI `--rewrite`, an uncaptured all-caps name is kept
  verbatim instead of being deleted.
- **Core hooks:** `Language::leaf_text_equivalent`,
  `is_literal_template_var` and `keep_unbound_template_var` default to the
  previous behavior and are forwarded by the CLI, Python and NAPI wrappers.
  Repeated metavariables now also compare nodes captured at different depths
  of a single-child wrapper chain.
- **Grammar limitations (0.26.6):** non-ASCII variable names (`$µA`, `$ÀB`)
  and type names fail to parse; `ForEach-Object { ... }` at the start of a
  statement is lexed as the `foreach` keyword (it parses inside a pipeline,
  e.g. `$x | ForEach-Object { ... }`); `param(...)` followed by statements on
  the same line inside a script block is an error.
- **Schemas:** `schemas/powershell_rule.json` added. The generator's field
  range was off by one and omitted every language's last field; the fix adds
  that field (e.g. `value`) to the existing language schemas.
  `schemas/languages.json` is not written by the generator and is unchanged.
- **Deferred:** NAPI (`napi-lang` subset) and WASM/playground builds, editor
  registrations, the documentation website and GitHub Action. Release binary
  size impact and non-Windows targets are verified by CI rather than locally.

## Outcome and scope

Add a built-in PowerShell parser for the Rust/CLI distribution, with dependable
searches, captures, fixes, YAML rules, Python bindings, and LSP diagnostics. This
is a plan, not an implementation. Work in the website, editor integrations,
GitHub Action, and optional language packages belongs to their respective
repositories and is identified separately below.

The implementation must first settle the **literal PowerShell variable versus
ast-grep metavariable** syntax. Merely registering a parser would accept rules
that silently mean something different from what their authors intended.
Release only the surfaces that have passed the same semantic contract.

Research baseline: this repository at
`5e2bef3a7ce2b9d254e85cb55d3c0d567423ee13`. The
[add-language guide](https://ast-grep.github.io/contributing/add-lang.html),
also linked from [.github/CONTRIBUTING.md](.github/CONTRIBUTING.md), is the
starting point. Some of its code examples describe older registration and
playground paths; use the current files linked below.

## Verified inputs and open risks

- **Grammar candidate, not an automatic selection:** Airbus CERT publishes
  [`tree-sitter-powershell` 0.26.6](https://docs.rs/crate/tree-sitter-powershell/0.26.6).
  Its [manifest](https://docs.rs/crate/tree-sitter-powershell/0.26.6/source/Cargo.toml)
  declares MIT and its [Rust binding](https://docs.rs/crate/tree-sitter-powershell/0.26.6/source/bindings/rust/lib.rs)
  exports `LANGUAGE`; review the
  [license](https://docs.rs/crate/tree-sitter-powershell/0.26.6/source/LICENSE)
  and preserve required notices in distributions. Evaluate maintenance,
  grammar quality, popularity, and size as requested by the add-language
  guide; its rough budget is for the **whole compressed binary**, under 10 MB,
  not a 10 MB allowance per grammar.
- The [published crate's source revision](https://docs.rs/crate/tree-sitter-powershell/0.26.6/source/.cargo_vcs_info.json)
  contains a [generated parser](https://raw.githubusercontent.com/airbus-cert/tree-sitter-powershell/65ea6919e816a7ac142acec7ee72757f89db8439/src/parser.c)
  declaring ABI 15. This checkout's [Cargo.lock](Cargo.lock) pins Tree-sitter
  0.27.2, whose documented [minimum ABI is 13](https://docs.rs/tree-sitter/0.27.2/tree_sitter/constant.MIN_COMPATIBLE_LANGUAGE_VERSION.html)
  and [maximum is 15](https://docs.rs/tree-sitter/0.27.2/tree_sitter/constant.LANGUAGE_VERSION.html).
  That is an ABI-range match, **not** evidence of complete language or
  platform compatibility. The grammar's
  [build script](https://raw.githubusercontent.com/airbus-cert/tree-sitter-powershell/65ea6919e816a7ac142acec7ee72757f89db8439/bindings/rust/build.rs)
  compiles an external C scanner and has special `wasm32-unknown-unknown`
  header/source requirements; verify native release targets and browser WASM
  independently.
- A prior small Rust experiment loaded the candidate and parsed input, but
  reported parse failures for `$µA` and `[µT]$x`; an unquoted `µA` argument
  parsed as a different node kind from `$A`. Reproduce these observations in
  repository tests. They do not prove that any marker or complete syntax is
  compatible. The grammar's [variable rules](https://docs.rs/crate/tree-sitter-powershell/0.26.6/source/grammar.js)
  are ASCII-oriented in places, while Microsoft documents broader
  [variable forms](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_variables),
  including Unicode, braced, and scoped names. Record actual corpus failures,
  supported PowerShell versions, and whether an upstream grammar fix, a
  maintained fork, or a documented limitation is necessary.
- PowerShell uses `$A`, `$HOME`, `$_`, `$$`, `$?`, and other dollar forms as
  real syntax; ast-grep also uses `$A`, `$_`, `$$A`, and `$$$ARGS` for
  metavariables. Identical unqualified spelling cannot simultaneously mean
  both a literal variable and a wildcard. The current generic preprocessing
  in [crates/language/src/lib.rs](crates/language/src/lib.rs) replaces `$`
  according to the immediately following character, not PowerShell lexical
  context. It can therefore affect ordinary variables, strings, or comments.
  The current [template fixer](crates/core/src/replacer/template.rs) also
  interprets metavariable-looking text in fixes and skips an unbound capture:
  treating literal `$HOME` as a capture could remove it in a
  [CLI rewrite](crates/cli/src/run.rs). YAML rules can instead reject an
  undefined capture during [fix validation](crates/config/src/check_var.rs),
  while [diagnostic messages](crates/config/src/rule_config.rs) use template
  substitution too. Test each path, not just searches, before allowing
  automatic rewrites.
- Named terminal matching is currently text-exact in
  [strictness.rs](crates/core/src/match_tree/strictness.rs), whereas
  PowerShell variable names are case-insensitive at runtime. Decide whether
  structural matching remains text-exact or deliberately gets a language-
  specific behavior; document and test whichever contract is chosen.

## Decisions required before implementation

1. **Pattern and replacement syntax:** choose and document an unambiguous
   way to express both an exact literal PowerShell variable and single,
   anonymous, unnamed, and multi-node captures. Evaluate an alternate
   metavariable sigil, an explicit literal escape, or a grammar-aware
   representation against actual AST node kinds and backward compatibility.
   The same choice must work in YAML rule patterns, constraints,
   transformations, replacement templates, diagnostics, and debug output.
   Do not assume changing `expando_char` to `µ` solves the problem; candidate
   sigils must also account for PowerShell `#` comments and `@` splatting.
2. **Grammar and coverage:** decide which PowerShell versions and constructs
   are supported; evaluate Windows PowerShell 5.1 and maintained PowerShell 7.x
   as initial targets. Decide the disposition of valid syntax the candidate
   does not parse. Prefer an upstream fix over an unowned fork when feasible.
   Make grammar/license/ABI/size results and limitations visible before release.
3. **Names and discovery:** propose canonical name `powershell`, then decide
   aliases and case policy. Target `.ps1`, `.psm1`, and `.psd1` discovery:
   Microsoft documents [scripts](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_scripts),
   [script modules](https://learn.microsoft.com/en-us/powershell/scripting/developer/module/how-to-write-a-powershell-script-module),
   and [module manifests](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_module_manifests).
   The grammar's own [metadata](https://raw.githubusercontent.com/airbus-cert/tree-sitter-powershell/65ea6919e816a7ac142acec7ee72757f89db8439/tree-sitter.json)
   lists only `ps1` and `psm1`, so require real `.psd1` parsing tests before
   promising manifest support. The current [extension lookup](crates/language/src/lib.rs)
   compares case-sensitively; decide whether uppercase extensions and other
   PowerShell file types are in scope.
4. **Existing custom-language users:** decide collision behavior and migration
   for configurations already named `powershell`. In
   [SgLang](crates/cli/src/lang/mod.rs), parsing a language name tries builtins
   before dynamic languages, while path inference tries configured globs and
   dynamic languages before builtin extensions. Without an explicit policy,
   the same name can resolve differently by entry point. Test both and
   document a migration such as renaming the custom registration if needed.
5. **Distribution:** decide whether PowerShell is built into NAPI or supplied
   as a dynamic package, whether playground support is part of the initial
   release, and whether bundled outline rules are included. A separate
   dynamic implementation is acceptable only if it can honor the chosen
   pattern/fix contract; otherwise do not advertise parity.

## Dependency-ordered implementation

### 0. Establish the compatibility gate

- [ ] Reproduce the prior `µ` observations and inspect parse errors, node
  kinds, and byte ranges for representative, hand-written PowerShell samples.
  Compare validity, not identical AST shapes, with Microsoft's
  [`Parser.ParseInput`](https://learn.microsoft.com/en-us/dotnet/api/system.management.automation.language.parser.parseinput)
  for the selected versions. Include scripts, modules, and a real manifest
  shape; parsing must not require executing the scripts.
- [ ] Confirm candidate version, license notices, maintenance, ABI, scanner
  compilation, and compressed release-binary size delta on the same target
  and profile. Exercise the scanner on Windows, Linux, and macOS release
  targets, and independently test any intended WASM build/runtime. Record
  unsupported syntax and the disposition of grammar bugs; open an upstream
  issue or patch where appropriate.
- [ ] Write the chosen pattern/fix syntax contract and its positive, negative,
  and migration examples. Require literal variables and captures to coexist
  in one pattern and one fix without silently changing source text.

### 1. Register the built-in parser and file types

- [ ] Add the grammar as an **optional** dependency and include it in
  `builtin-parser` in [crates/language/Cargo.toml](crates/language/Cargo.toml);
  update [Cargo.lock](Cargo.lock). Follow the current per-parser
  `conditional_lang!` pattern in [parsers.rs](crates/language/src/parsers.rs),
  including the feature-disabled path. Do not copy the guide's obsolete
  `parser_implementation` module verbatim.
- [ ] Implement the language type/trait and wire `SupportLang`, method
  dispatch, `all_langs`, aliases, extensions, and file-type filters in
  [crates/language/src/lib.rs](crates/language/src/lib.rs). Add metadata and
  inference tests, including non-default feature builds and the chosen
  `.psd1`/uppercase-extension policy.
- [ ] Test explicit `-l powershell`, inferred paths, `languageGlobs`, stdin,
  rule-language parsing, and help/new-rule language lists. Test explicit-name
  versus inferred-path behavior for a pre-existing dynamic `powershell`
  registration and preserve or migrate it according to decision 4.

### 2. Implement patterns, captures, and safe fixes

- [ ] Implement the decided syntax across preprocessing, parser-node
  extraction, `MetaVariable`, and `Pattern` in
  [core/language.rs](crates/core/src/language.rs),
  [core/meta_var.rs](crates/core/src/meta_var.rs), and
  [core/matcher/pattern.rs](crates/core/src/matcher/pattern.rs). Verify
  placeholders occupy compatible AST positions in command argument,
  expression, variable, type, and list contexts; test contextual selectors
  where a standalone snippet is ambiguous. Retain all existing-language
  behavior.
- [ ] Implement and test literal/capture behavior in both
  [template](crates/core/src/replacer/template.rs) and
  [structural](crates/core/src/replacer/structural.rs) rewriting, and in
  [transform parsing](crates/config/src/transform/trans.rs). Assert the exact
  replacement bytes, including literal `$HOME`, escaped/interpolated strings,
  and unbound names; a preview and an applied edit must agree. Check
  [pattern debug output](crates/core/src/matcher/pattern.rs), which currently
  formats captures using `$`, if the user-facing sigil changes.
- [ ] If the implementation overrides `Language::extract_meta_var`, forward it
  through `SupportLang` **and** the language wrappers:
  [SgLang](crates/cli/src/lang/mod.rs),
  [PyLang](crates/pyo3/src/py_lang.rs), and
  [NapiLang](crates/napi/src/napi_lang.rs). These wrappers currently forward
  other language methods but do not override extraction. Exercise equivalent
  patterns and fixes through each supported wrapper; otherwise direct Rust
  behavior can differ from CLI/LSP or bindings.
- [ ] Add regression tests for unaffected languages, shared preprocessing,
  captures, transform sources, and replacement templates, particularly if any
  shared trait or template API changes.

### 3. Complete in-repository product surfaces

- [ ] **CLI and config:** add `run`, JSON output, `scan`, `test`, `new`, and
  `--debug-query` tests using the selected alias and syntax. Include
  `ruleDirs`, fix preview/application and undefined-variable validation,
  constraints, transforms, message interpolation, capture-based labels,
  ignored files, extension inference, and `languageGlobs`. Verify that a rule
  aimed at PowerShell does not scan an unrelated file. Use PowerShell-safe
  quoting in examples so the invoking shell does not expand the pattern.
- [ ] **Python:** build/install the local wheel in an isolated test environment
  and add [crates/pyo3/tests](crates/pyo3/tests) coverage for builtin
  `SgRoot`, find/captures, rules, and edits. The Python wrapper's
  [dynamic-language types](crates/pyo3/ast_grep_py/__init__.py) expose sigil
  configuration; update API/types only if the selected design needs it.
- [ ] **LSP:** extend [basic LSP tests](crates/lsp/tests/basic.rs) for `.ps1`
  and the other accepted extensions: `didOpen`, `didChange`, diagnostics,
  code actions and edits, including CRLF and non-BMP text with correct UTF-16
  positions. [The server](crates/lsp/src/lib.rs) infers from the URI; this
  does not register PowerShell with editor clients.
- [ ] **Schemas:** add PowerShell to
  [xtask/src/schema.rs](xtask/src/schema.rs), generate and commit
  `schemas/powershell_rule.json`, and review `schemas/rule.json`. The
  generator canonicalizes the destination before writing, so create the new
  output file first or change it to permit a new file. Fix and test its
  existing `1..lang.field_count()` loop: Tree-sitter field IDs are one-based
  and this omits the final field; for this parser ID 14 is `value`. Use the
  inclusive range and review any resulting existing-language schema diffs.
  Run generation twice and require no second diff. Investigate consumers of
  [schemas/languages.json](schemas/languages.json), which this generator does
  not write, and update it only if appropriate. The generic
  [project schema](schemas/project.json) needs changes only if new
  configuration properties are introduced.
- [ ] **Outline — explicit scope choice:** parsing alone does not provide
  default outline rules. If included, add and bundle rules for PowerShell
  functions, classes, and relevant members in
  [default_rule.rs](crates/outline/src/default_rule.rs), with
  [outline tests](crates/outline/tests/default_outline_rules.rs). Otherwise
  document that outlines are deferred; custom outline rules remain a
  separate capability.

### 4. Conditional binding and browser work

- [ ] **NAPI, if advertised:** the current
  [napi-lang feature](crates/language/Cargo.toml),
  [NapiLang](crates/napi/src/napi_lang.rs), and
  [TypeScript language types](crates/napi/types/lang.d.ts) expose a small
  built-in subset, not every CLI language. If making PowerShell built in,
  update the feature, enum/name handling, generated types, build/tests, and
  measure package impact. If using a dynamic parser instead, prove the
  selected pattern/fix contract is expressible through the registration API;
  a static `expandoChar` alone may not be sufficient.
- [ ] **WASM, if advertised:** [WasmLang](crates/wasm/src/wasm_lang.rs) uses
  dynamically registered parsers, its own preprocessing, and registration
  exposes `expandoChar` but not `metaVarChar`
  ([API](crates/wasm/src/lib.rs)). If the chosen design cannot be represented,
  extend the API/types and implementation rather than claiming parity.
  Test registration, parsing, captures, fixes, and `dumpPattern`; the latter
  currently reconstructs marker text with a hard-coded `$`. Ensure the actual
  PowerShell parser WASM, including its scanner, loads in the runtime.

## Required test matrix

Add focused tests at the lowest affected layer and repeat representative
end-to-end cases through each advertised distribution:

| Area | Cases and assertions |
| --- | --- |
| Grammar | `.ps1`, `.psm1`, and candidate `.psd1`; pipelines, commands and named arguments, functions, classes/methods, typed parameters, script blocks, splatting, arrays and hash tables. Assert expected nodes and no unexpected `ERROR`/missing nodes in the supported corpus. |
| Variables | `$a`, `$A`, `$HOME`, `$_`, `$PSItem`, `$$`, `$?`, `$env:Path`, scoped and braced variables, Unicode names, and the prior `µ` examples. Exercise the chosen case and literal/marker policies, including negative matches. |
| Lexical contexts | Single/double-quoted strings, interpolations, escaped dollars, both here-string forms, line/block comments, and ordinary text that resembles a marker. Confirm preprocessing does not corrupt literals or comments. |
| Matching | Literal variable and wildcard in the same pattern; repeated capture equality, anonymous/unnamed captures, zero/one/many list captures, contextual patterns, kinds/fields, constraints, and no false positives. Check the selected case-sensitivity contract. |
| Rewriting | Capture substitution beside literal variables; transforms and structural fixes; missing/unbound captures and escaped text; exact output and source ranges for both preview and application. |
| Integration | YAML rules and tests; CLI discovery and custom-language collisions; LSP incremental changes, diagnostics and UTF-16 edits; Python; conditional NAPI/WASM; LF/CRLF and non-ASCII source; regressions for existing languages. |

## Documentation and downstream follow-up

The [contribution guide](https://ast-grep.github.io/contributing/how-to.html)
identifies the separate documentation, Action, and editor repositories. Treat
these as tracked follow-ups rather than assuming an LSP server change updates
their clients:

| Surface | Required action / release relationship |
| --- | --- |
| Public docs — separate [`ast-grep.github.io`](https://github.com/ast-grep/ast-grep.github.io) | **Required for a public PowerShell release:** update [`website/reference/languages.md`](https://github.com/ast-grep/ast-grep.github.io/blob/main/website/reference/languages.md), describe aliases/extensions, version/grammar limitations, exact literal/capture/fix syntax, case behavior, and custom-language migration. Add tested PowerShell-safe CLI and YAML examples. Qualify the general `$` wildcard description in this repository's [README](README.md) if the chosen PowerShell syntax is an exception. |
| Playground — same separate website repo | **Conditional on offering playground support:** update current [`website/_data/parsers.ts`](https://github.com/ast-grep/ast-grep.github.io/blob/main/website/_data/parsers.ts), [`website/src/components/astGrep/lang.ts`](https://github.com/ast-grep/ast-grep.github.io/blob/main/website/src/components/astGrep/lang.ts), [`src/wasm_lang.rs`](https://github.com/ast-grep/ast-grep.github.io/blob/main/src/wasm_lang.rs), and `website/public/parsers` as applicable. The website has its own pattern preprocessing and [separately pinned core](https://github.com/ast-grep/ast-grep.github.io/blob/main/Cargo.toml), so port the chosen contract and update dependencies as needed; changing this repository's WASM crate is not automatically sufficient. The [parser downloader](https://github.com/ast-grep/ast-grep.github.io/blob/main/website/public/parsers/downloadParsers.mjs) needs a real published WASM URL/version, or build and supply the file. Verify browser runtime compatibility and actual patterns and rewrites, not just the dropdown. The guide's older `website/src/components/lang.ts` path is stale. |
| Dynamic NAPI package — separate [`ast-grep/langs`](https://github.com/ast-grep/langs) | **Conditional:** publish/test a PowerShell package if that distribution is chosen, with its native binaries, declaration and runtime configuration. Verify it supports the selected syntax, or extend the API first. |
| VS Code — separate [`ast-grep-vscode`](https://github.com/ast-grep/ast-grep-vscode) | **Follow-up:** the [LSP selector](https://github.com/ast-grep/ast-grep-vscode/blob/main/src/extension/lsp.ts) uses a wildcard, but the explicit [search dropdown](https://github.com/ast-grep/ast-grep-vscode/blob/main/src/webview/SearchSidebar/SearchWidgetContainer/LangSelect.tsx) comes from a static [icons list](https://github.com/ast-grep/ast-grep-vscode/blob/main/src/webview/SearchSidebar/SearchResultList/Icon.tsx) without PowerShell. Update/test the search list/icon if offering it; verify diagnostics, code actions, and search with a supporting CLI. |
| Neovim — separate integrations | **Follow-up:** [`nvim-lspconfig`'s ast-grep config](https://github.com/neovim/nvim-lspconfig/blob/master/lsp/ast_grep.lua) does not currently list `ps1`; Neovim's [filetype mappings](https://github.com/neovim/neovim/blob/master/runtime/lua/vim/filetype.lua) map `.ps1`, `.psm1`, and `.psd1` to `ps1`. Document a local filetype override and propose an upstream change. [`coc-ast-grep`](https://github.com/yaegassy/coc-ast-grep/blob/master/package.json) currently activates for TypeScript and pins CLI 0.45.3; assess activation and server version. [`telescope-sg`](https://github.com/Marskey/telescope-sg/blob/main/lua/telescope/_extensions/ast_grep.lua) forwards a free-form language, so verify alias and JSON output before requesting a code change. |
| GitHub Action — separate [`ast-grep/action`](https://github.com/ast-grep/action) | **Release verification:** its [entry point](https://github.com/ast-grep/action/blob/main/src/index.ts) installs a requested or latest CLI and invokes `scan --format github`. Test a released PowerShell-capable CLI and annotations on a `.ps1` fixture; tell users pinning an older CLI to update. Do not presume an Action code change is needed. |

## Validation and release acceptance

Name new tests with `powershell` so the following focused commands select them
from the repository root; combine or adjust selectors to the actual test
locations. Use the existing CI environments and installed tools:

```powershell
cargo check -p ast-grep-language --no-default-features
cargo check -p ast-grep-language --no-default-features --features napi-lang
cargo test -p ast-grep-language powershell
cargo test -p ast-grep-core
cargo test -p ast-grep-config
cargo test -p ast-grep --test run_test --test scan_test --test verify_test powershell
cargo test -p ast-grep-lsp --test basic powershell
cargo test -p ast-grep-outline powershell
cargo test -p xtask
cargo xtask schema
cargo fmt --all -- --check
cargo test
cargo clippy --all-targets --all-features --workspace --release --locked -- -D clippy::all
```

Run schema generation a second time and check for no additional diff. Run
Python wheel tests and, when changed, NAPI `yarn build:debug`/`yarn test` and
the [WASM workflow](.github/workflows/wasm.yml) tests. Use the existing
[release](.github/workflows/release.yml),
[Python](.github/workflows/pyo3.yml), and
[NAPI](.github/workflows/napi.yml) matrices to check supported platforms.
Also perform CLI smoke tests for literal search, previewed/applied rewrite,
configured scan, and LSP startup with repository-local fixtures.

Release acceptance:

- [ ] A documented grammar, license, ABI, platform/size result, supported
  version/corpus, and explicit disposition for unsupported valid syntax.
- [ ] Unambiguous literal-versus-capture behavior and safe fixes verified for
  the matrix above, with no silent loss of PowerShell variables or regressions
  to existing languages.
- [ ] Parser registration, file discovery, rule configuration, schemas, Python,
  and LSP work end to end; custom-language collision behavior is tested and
  migration is documented.
- [ ] Each advertised optional surface has equivalent tests; deferred NAPI,
  outline, playground, editor, or Action work is identified rather than
  implied to be complete. Public documentation is ready for the release.
