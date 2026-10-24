//! The diagnostics of one load, bounded while they are collected (Q-9): an input that yields
//! errors by the hundred thousand must not hold them all before the last thousand are cut.

use forager_style_ir::Diagnostic;

use crate::MAX_DIAGNOSTICS;

#[derive(Default)]
pub struct Report {
    kept: Vec<Diagnostic>,
    dropped: usize,
    failed: bool,
}

impl Report {
    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.failed |= diagnostic.is_error();
        if self.kept.len() < MAX_DIAGNOSTICS {
            self.kept.push(diagnostic);
        } else {
            self.dropped += 1;
        }
    }

    /// Whether any error was reported, kept or dropped.
    pub fn failed(&self) -> bool {
        self.failed
    }

    /// The kept diagnostics, and a last one that counts the dropped.
    pub fn finish(mut self) -> Vec<Diagnostic> {
        if self.dropped > 0 {
            self.kept.push(Diagnostic::warning(
                "diagnostics-truncated",
                format!("{} more diagnostics are not shown", self.dropped),
                None,
            ));
        }
        self.kept
    }
}
