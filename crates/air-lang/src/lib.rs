//! Languages as data. A module brings a [`LangDef`] (JSON); [`Language`] compiles it and gives the editor what it
//! needs - highlighting, bracket matching, an outline, folding, indentation - without any code of its own.

mod def;
mod engine;
mod set;

pub use def::{IndentDef, LangDef, OutlineDef, RuleDef};
pub use engine::{Continuation, FoldKind, IndentMode, Language, class_named, line_starts};
pub use set::LanguageSet;

#[cfg(test)]
mod tests;
