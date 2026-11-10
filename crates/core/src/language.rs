use crate::matcher::PatternBuilder;
use crate::meta_var::{MetaVariable, extract_meta_var};
use crate::{Pattern, PatternError};
use std::borrow::Cow;
use std::path::Path;

/// Trait to abstract ts-language usage in ast-grep, which includes:
/// * which character is used for meta variable.
/// * if we need to use other char in meta var for parser at runtime
/// * pre process the Pattern code.
pub trait Language: Clone + 'static {
  /// normalize pattern code before matching
  /// e.g. remove expression_statement, or prefer parsing {} to object over block
  fn pre_process_pattern<'q>(&self, query: &'q str) -> Cow<'q, str> {
    Cow::Borrowed(query)
  }

  /// Configure meta variable special character
  /// By default $ is the metavar char, but in PHP it can be #
  #[inline]
  fn meta_var_char(&self) -> char {
    '$'
  }

  /// Some language does not accept $ as the leading char for identifiers.
  /// We need to change $ to other char at run-time to make parser happy, thus the name expando.
  /// By default this is the same as meta_var char so replacement is done at runtime.
  #[inline]
  fn expando_char(&self) -> char {
    self.meta_var_char()
  }

  /// extract MetaVariable from a given source string
  /// At runtime we need to use expand_char
  fn extract_meta_var(&self, source: &str) -> Option<MetaVariable> {
    extract_meta_var(source, self.expando_char())
  }

  /// Whether two named leaf nodes of `kind_id` with different texts are still equivalent.
  /// It is only consulted after an exact text comparison fails, so the default never
  /// relaxes matching. Languages can override it, e.g. for case-insensitive identifiers.
  #[inline]
  fn leaf_text_equivalent(&self, _kind_id: u16, _goal: &str, _candidate: &str) -> bool {
    false
  }

  /// Whether `$NAME` in a fix template is literal text rather than a meta variable.
  /// `name` excludes the meta var char(s) and `rest` is the template text after the name.
  /// Useful when the language itself uses `$` names.
  #[inline]
  fn is_literal_template_var(&self, _name: &str, _rest: &str) -> bool {
    false
  }

  /// Whether a fix template keeps a meta variable without a captured value verbatim.
  /// By default an unbound meta variable is replaced with an empty string.
  #[inline]
  fn keep_unbound_template_var(&self) -> bool {
    false
  }
  /// Return the file language from path. Return None if the file type is not supported.
  fn from_path<P: AsRef<Path>>(_path: P) -> Option<Self> {
    // TODO: throw panic here if not implemented properly?
    None
  }

  fn kind_to_id(&self, kind: &str) -> u16;
  fn field_to_id(&self, field: &str) -> Option<u16>;
  fn build_pattern(&self, builder: &PatternBuilder) -> Result<Pattern, PatternError>;
}

#[cfg(test)]
pub use test::*;

#[cfg(test)]
mod test {
  use super::*;
  use crate::tree_sitter::{LanguageExt, StrDoc, TSLanguage};

  #[derive(Clone)]
  pub struct Tsx;
  impl Language for Tsx {
    fn kind_to_id(&self, kind: &str) -> u16 {
      let ts_lang: TSLanguage = tree_sitter_typescript::LANGUAGE_TSX.into();
      ts_lang.id_for_node_kind(kind, /* named */ true)
    }
    fn field_to_id(&self, field: &str) -> Option<u16> {
      self
        .get_ts_language()
        .field_id_for_name(field)
        .map(|f| f.get())
    }
    fn build_pattern(&self, builder: &PatternBuilder) -> Result<Pattern, PatternError> {
      builder.build(|src| StrDoc::try_new(src, self.clone()))
    }
  }
  impl LanguageExt for Tsx {
    fn get_ts_language(&self) -> TSLanguage {
      tree_sitter_typescript::LANGUAGE_TSX.into()
    }
  }
}
