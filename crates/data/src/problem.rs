use serde::{Deserialize, Serialize};

use crate::kind::Kind;
use crate::model::Source;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Severity {
    Error,
    Warning,
}

/// A load or validation problem, located as precisely as possible.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Problem {
    pub severity: Severity,
    /// Pack and file.
    pub source: Option<Source>,
    /// Line and column (1-based) for parse errors.
    pub line: Option<(usize, usize)>,
    pub kind: Option<Kind>,
    pub id: Option<String>,
    /// Field path inside the entry, e.g. `parts[3].material` or `bonuses[0].stat`.
    pub field: Option<String>,
    pub message: String,
}

impl Problem {
    pub fn error(message: impl Into<String>) -> Self {
        Problem {
            severity: Severity::Error,
            source: None,
            line: None,
            kind: None,
            id: None,
            field: None,
            message: message.into(),
        }
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Problem {
            severity: Severity::Warning,
            ..Problem::error(message)
        }
    }

    pub fn at(mut self, source: &Source) -> Self {
        self.source = Some(source.clone());
        self
    }

    pub fn entry(mut self, kind: Kind, id: &str) -> Self {
        self.kind = Some(kind);
        self.id = Some(id.to_string());
        self
    }

    pub fn field(mut self, field: impl Into<String>) -> Self {
        self.field = Some(field.into());
        self
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        })?;
        if let Some(s) = &self.source {
            write!(f, ": {s}")?;
            if let Some((l, c)) = self.line {
                write!(f, ":{l}:{c}")?;
            }
        }
        if let (Some(k), Some(id)) = (self.kind, &self.id) {
            write!(f, ": {k} `{id}`")?;
        }
        if let Some(field) = &self.field {
            write!(f, ", field `{field}`")?;
        }
        write!(f, ": {}", self.message)
    }
}

/// Splits problems into `Err(all)` if any is an error, otherwise `Ok(warnings)`.
pub fn errors_or_warnings(problems: Vec<Problem>) -> Result<Vec<Problem>, Vec<Problem>> {
    if problems.iter().any(Problem::is_error) {
        Err(problems)
    } else {
        Ok(problems)
    }
}
