//! A count held to a bound of Q-9: what a load reads, and what the defaults of `VALIDATION` add to
//! its expressions. It charges, stops one past its bound, and tells its caller the one charge that
//! passes the bound, so that the load reports it once and a count past the bound never wraps
//! (docs/CONVENTIONS.md).

/// A count of bytes held to a bound.
#[derive(Debug)]
pub(crate) struct Budget {
    spent: usize,
    bound: usize,
}

/// What a charge did to a [`Budget`].
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Charged {
    /// The count stays within the bound.
    Within,
    /// This charge passed the bound: the one to report.
    PassedNow,
    /// The bound was passed before; nothing is charged.
    Passed,
}

impl Budget {
    pub(crate) const fn new(bound: usize) -> Self {
        Self { spent: 0, bound }
    }

    /// Whether a charge has passed the bound.
    pub(crate) fn passed(&self) -> bool {
        self.spent > self.bound
    }

    /// Charges `amount`, unless the bound was passed before. The count stops one past the bound.
    pub(crate) fn charge(&mut self, amount: usize) -> Charged {
        if self.passed() {
            return Charged::Passed;
        }
        self.spent = self.spent.saturating_add(amount);
        if self.passed() {
            self.spent = self.bound.saturating_add(1);
            return Charged::PassedNow;
        }
        Charged::Within
    }

    /// Whether `len` bytes fit within the bound on their own, as one expression with its defaults
    /// must.
    pub(crate) fn admits(&self, len: usize) -> bool {
        len <= self.bound
    }

    /// The bound.
    pub(crate) fn bound(&self) -> usize {
        self.bound
    }
}

#[cfg(test)]
mod tests {
    use super::{Budget, Charged};

    #[test]
    fn a_budget_reports_the_one_charge_that_passes_it_and_stops() {
        // Verifies: Q-9
        let mut budget = Budget::new(10);
        assert_eq!(budget.charge(10), Charged::Within);
        assert!(!budget.passed());
        assert_eq!(budget.charge(1), Charged::PassedNow);
        assert!(budget.passed());
        assert_eq!(budget.charge(usize::MAX), Charged::Passed);
        assert_eq!(budget.charge(0), Charged::Passed);
        let mut budget = Budget::new(10);
        assert_eq!(budget.charge(usize::MAX), Charged::PassedNow, "no wrap");
        assert!(budget.admits(10) && !budget.admits(11));
    }
}
