//! One UML multiplicity convention for OWL counts and SHACL constraints.
//!
//! Equal bounds collapse to one number (`1`, not `1..1`), an unconstrained
//! range is `*` (not `0..*`), and an upper bound of zero is no multiplicity
//! at all: it is a prohibition, which the diagram states as a class
//! constraint such as `{no title}` instead of a `[0..0]` member.
use std::fmt;

/// Cardinality bounds that are not a prohibition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Multiplicity {
    lower: u64,
    upper: Option<u64>,
}
/// What a pair of cardinality bounds means in a class diagram.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Cardinality {
    /// No value may exist: drawn as a class constraint.
    Prohibited,
    /// Values are bounded as stated.
    Bounded(Multiplicity),
}
impl Cardinality {
    /// Bounds from a lower count and an optional upper count. An upper bound
    /// below the lower bound is unsatisfiable and has no UML multiplicity.
    pub fn try_new(lower: u64, upper: Option<u64>) -> Option<Self> {
        match upper {
            Some(0) => Some(Self::Prohibited),
            Some(upper) if upper < lower => None,
            _ => Some(Self::Bounded(Multiplicity { lower, upper })),
        }
    }

    pub fn at_least(lower: u64) -> Self {
        Self::Bounded(Multiplicity { lower, upper: None })
    }

    pub fn exactly(count: u64) -> Self {
        Self::try_new(count, Some(count)).unwrap_or(Self::Prohibited)
    }

    pub fn at_most(upper: u64) -> Self {
        Self::try_new(0, Some(upper)).unwrap_or(Self::Prohibited)
    }

    /// The multiplicity to draw; `None` for a prohibition.
    pub fn multiplicity(self) -> Option<Multiplicity> {
        match self {
            Self::Prohibited => None,
            Self::Bounded(multiplicity) => Some(multiplicity),
        }
    }
}
impl Multiplicity {
    /// Whether these bounds constrain nothing (`*`).
    pub fn is_unconstrained(self) -> bool {
        self.lower == 0 && self.upper.is_none()
    }

    /// Whether at least one value is required.
    pub fn requires_value(self) -> bool { self.lower > 0 }
}
impl fmt::Display for Multiplicity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.lower, self.upper) {
            (0, None) => formatter.write_str("*"),
            (lower, None) => write!(formatter, "{lower}..*"),
            (lower, Some(upper)) if lower == upper => {
                write!(formatter, "{lower}")
            },
            (lower, Some(upper)) => write!(formatter, "{lower}..{upper}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Cardinality;

    fn drawn(lower: u64, upper: Option<u64>) -> Option<String> {
        Cardinality::try_new(lower, upper)?
            .multiplicity()
            .map(|m| m.to_string())
    }

    #[test]
    fn multiplicities_follow_one_uml_convention() {
        assert_eq!(drawn(1, Some(1)).as_deref(), Some("1"));
        assert_eq!(drawn(3, Some(3)).as_deref(), Some("3"));
        assert_eq!(drawn(0, None).as_deref(), Some("*"));
        assert_eq!(drawn(1, None).as_deref(), Some("1..*"));
        assert_eq!(drawn(0, Some(1)).as_deref(), Some("0..1"));
        assert_eq!(drawn(2, Some(5)).as_deref(), Some("2..5"));
        assert_eq!(drawn(3, Some(1)), None);
    }

    #[test]
    fn an_upper_bound_of_zero_is_a_prohibition_not_a_multiplicity() {
        for cardinality in [
            Cardinality::try_new(0, Some(0)).unwrap(),
            Cardinality::exactly(0),
            Cardinality::at_most(0),
        ] {
            assert_eq!(cardinality, Cardinality::Prohibited);
            assert_eq!(cardinality.multiplicity(), None);
        }
    }
}
