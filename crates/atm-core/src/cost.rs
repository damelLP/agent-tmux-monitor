//! Money and cost tracking value objects.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Add, AddAssign};

/// Represents a monetary amount in USD.
///
/// Internally stored as microdollars (millionths of a dollar) for precision.
/// Avoids floating-point errors in cost accumulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Money {
    /// Amount in microdollars (1 USD = 1,000,000 microdollars)
    microdollars: i64,
}

impl Money {
    /// One dollar in microdollars.
    const MICRODOLLARS_PER_DOLLAR: i64 = 1_000_000;

    /// Creates Money from a USD dollar amount.
    pub fn from_usd(dollars: f64) -> Self {
        let microdollars = (dollars * Self::MICRODOLLARS_PER_DOLLAR as f64).round() as i64;
        Self { microdollars }
    }

    /// Creates Money from microdollars.
    pub fn from_microdollars(microdollars: i64) -> Self {
        Self { microdollars }
    }

    /// Creates a zero Money value.
    pub const fn zero() -> Self {
        Self { microdollars: 0 }
    }

    /// Returns the amount in USD as a float.
    pub fn as_usd(&self) -> f64 {
        self.microdollars as f64 / Self::MICRODOLLARS_PER_DOLLAR as f64
    }

    /// Returns the amount in microdollars.
    pub fn as_microdollars(&self) -> i64 {
        self.microdollars
    }

    /// Returns true if the amount is zero.
    pub fn is_zero(&self) -> bool {
        self.microdollars == 0
    }

    /// Adds another Money value.
    pub fn add(&self, other: Money) -> Self {
        Self {
            microdollars: self.microdollars.saturating_add(other.microdollars),
        }
    }

    /// Formats the amount for display.
    ///
    /// Returns format like "$0.35", "$1.50", "$12.34"
    pub fn format(&self) -> String {
        let dollars = self.as_usd();
        if dollars < 0.01 && dollars > 0.0 {
            format!("${dollars:.4}")
        } else if dollars < 10.0 {
            format!("${dollars:.2}")
        } else if dollars < 100.0 {
            format!("${dollars:.1}")
        } else {
            format!("${dollars:.0}")
        }
    }

    /// Formats the amount compactly for narrow displays.
    ///
    /// Returns format like "35c", "$1.5", "$12"
    pub fn format_compact(&self) -> String {
        let dollars = self.as_usd();
        if dollars < 0.01 && dollars > 0.0 {
            let cents = dollars * 100.0;
            format!("{cents:.1}c")
        } else if dollars < 1.0 {
            let cents = (dollars * 100.0).round() as i32;
            format!("{cents}c")
        } else if dollars < 10.0 {
            format!("${dollars:.1}")
        } else {
            format!("${dollars:.0}")
        }
    }
}

impl Add for Money {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            microdollars: self.microdollars.saturating_add(other.microdollars),
        }
    }
}

impl AddAssign for Money {
    fn add_assign(&mut self, other: Self) {
        self.microdollars = self.microdollars.saturating_add(other.microdollars);
    }
}

impl Serialize for Money {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Serialize as USD float for JSON compatibility
        serializer.serialize_f64(self.as_usd())
    }
}

impl<'de> Deserialize<'de> for Money {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let dollars = f64::deserialize(deserializer)?;
        Ok(Money::from_usd(dollars))
    }
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_money_precision() {
        let a = Money::from_usd(0.001);
        let b = Money::from_usd(0.001);
        let sum = a + b;
        assert!((sum.as_usd() - 0.002).abs() < 0.0001);
    }

    #[test]
    fn test_money_formatting() {
        assert_eq!(Money::from_usd(0.005).format(), "$0.0050");
        assert_eq!(Money::from_usd(0.35).format(), "$0.35");
        assert_eq!(Money::from_usd(1.50).format(), "$1.50");
        assert_eq!(Money::from_usd(12.34).format(), "$12.3");
        assert_eq!(Money::from_usd(150.00).format(), "$150");
    }

    #[test]
    fn test_money_compact_formatting() {
        assert_eq!(Money::from_usd(0.35).format_compact(), "35c");
        assert_eq!(Money::from_usd(1.50).format_compact(), "$1.5");
        assert_eq!(Money::from_usd(12.34).format_compact(), "$12");
    }

    #[test]
    fn test_money_zero() {
        assert!(Money::zero().is_zero());
        assert!(!Money::from_usd(0.01).is_zero());
    }
}
