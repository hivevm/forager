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

impl Budget {
    pub(crate) const fn new(bound: usize) -> Self {
        Self { spent: 0, bound }
    }

    /// Whether a charge has passed the bound: what a caller asks before the work it would charge.
    pub(crate) fn passed(&self) -> bool {
        self.spent > self.bound
    }

    /// Charges `amount`, and answers whether this charge is the one that passed the bound, the
    /// one to report: once, as the count stops one past the bound and a later charge adds nothing.
    #[must_use]
    pub(crate) fn charge(&mut self, amount: usize) -> bool {
        if self.passed() {
            return false;
        }
        self.spent = self.spent.saturating_add(amount);
        if self.passed() {
            self.spent = self.bound.saturating_add(1);
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::Budget;

    #[test]
    fn a_budget_reports_the_one_charge_that_passes_it_and_stops() {
        // Verifies: Q-9
        let mut budget = Budget::new(10);
        assert!(!budget.charge(10));
        assert!(!budget.passed());
        assert!(budget.charge(1), "the charge that passes the bound");
        assert!(budget.passed());
        assert!(!budget.charge(usize::MAX), "reported once");
        assert!(!budget.charge(0));
        assert!(budget.passed());
        let mut budget = Budget::new(10);
        assert!(budget.charge(usize::MAX), "no wrap");
    }
}
