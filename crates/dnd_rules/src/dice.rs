//! Dice expressions such as `1d20+5`, `2d6+3`, `4d8`, `d100`, `2d6+1d4-1`.
//!
//! The parser is strict and bounded so untrusted client input cannot request
//! absurd rolls (see `MAX_*` limits).

use crate::rng::DiceRng;
use serde::{Deserialize, Serialize};
use std::fmt;

pub const MAX_DICE_PER_TERM: u32 = 100;
pub const MAX_SIDES: u32 = 1000;
pub const MAX_TERMS: usize = 16;
pub const MAX_EXPR_LEN: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiceTerm {
    pub count: u32,
    pub sides: u32,
    /// `true` when the term is subtracted (e.g. `1d6-1d4`).
    pub negative: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DiceExpr {
    pub terms: Vec<DiceTerm>,
    pub modifier: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DiceParseError {
    #[error("empty dice expression")]
    Empty,
    #[error("expression too long")]
    TooLong,
    #[error("invalid token near '{0}'")]
    Invalid(String),
    #[error("too many dice or sides (max {MAX_DICE_PER_TERM}d{MAX_SIDES}, {MAX_TERMS} terms)")]
    OutOfBounds,
}

impl DiceExpr {
    pub fn dice(count: u32, sides: u32) -> Self {
        Self {
            terms: vec![DiceTerm {
                count,
                sides,
                negative: false,
            }],
            modifier: 0,
        }
    }
    pub fn with_modifier(mut self, m: i32) -> Self {
        self.modifier += m;
        self
    }

    /// Parses expressions made of `NdS` and integer terms joined by `+`/`-`. Whitespace is ignored.
    pub fn parse(input: &str) -> Result<Self, DiceParseError> {
        if input.len() > MAX_EXPR_LEN {
            return Err(DiceParseError::TooLong);
        }
        let s: String = input
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
            .to_ascii_lowercase();
        if s.is_empty() {
            return Err(DiceParseError::Empty);
        }
        let mut expr = DiceExpr::default();
        let bytes = s.as_bytes();
        let mut i = 0;
        let mut first = true;
        while i < bytes.len() {
            let mut negative = false;
            if bytes[i] == b'+' || bytes[i] == b'-' {
                negative = bytes[i] == b'-';
                i += 1;
            } else if !first {
                return Err(DiceParseError::Invalid(s[i..].to_string()));
            }
            first = false;
            let start = i;
            while i < bytes.len() && bytes[i] != b'+' && bytes[i] != b'-' {
                i += 1;
            }
            let tok = &s[start..i];
            if tok.is_empty() {
                return Err(DiceParseError::Invalid(
                    s[start.saturating_sub(1)..].to_string(),
                ));
            }
            if let Some((c, sd)) = tok.split_once('d') {
                let count: u32 = if c.is_empty() {
                    1
                } else {
                    c.parse().map_err(|_| DiceParseError::Invalid(tok.into()))?
                };
                let sides: u32 = sd
                    .parse()
                    .map_err(|_| DiceParseError::Invalid(tok.into()))?;
                if count == 0 || sides == 0 || count > MAX_DICE_PER_TERM || sides > MAX_SIDES {
                    return Err(DiceParseError::OutOfBounds);
                }
                expr.terms.push(DiceTerm {
                    count,
                    sides,
                    negative,
                });
                if expr.terms.len() > MAX_TERMS {
                    return Err(DiceParseError::OutOfBounds);
                }
            } else {
                let v: i32 = tok
                    .parse()
                    .map_err(|_| DiceParseError::Invalid(tok.into()))?;
                if v > 10_000 {
                    return Err(DiceParseError::OutOfBounds);
                }
                expr.modifier += if negative { -v } else { v };
            }
        }
        Ok(expr)
    }

    /// Rolls the expression. With `critical`, every die is rolled twice as many times
    /// ("Critical Hits": roll the attack's damage dice twice, add them together, then add modifiers).
    pub fn roll(&self, rng: &mut dyn DiceRng, critical: bool) -> DiceRoll {
        let mut terms = Vec::with_capacity(self.terms.len());
        let mut total = self.modifier;
        for t in &self.terms {
            let n = if critical { t.count * 2 } else { t.count };
            let rolls: Vec<u32> = (0..n).map(|_| rng.roll_die(t.sides)).collect();
            let sum: i32 = rolls.iter().map(|v| *v as i32).sum();
            total += if t.negative { -sum } else { sum };
            terms.push(TermRoll { term: *t, rolls });
        }
        DiceRoll {
            expr: self.clone(),
            critical,
            terms,
            total,
        }
    }

    pub fn average(&self) -> f64 {
        let dice: f64 = self
            .terms
            .iter()
            .map(|t| {
                let a = t.count as f64 * (t.sides as f64 + 1.0) / 2.0;
                if t.negative { -a } else { a }
            })
            .sum();
        dice + self.modifier as f64
    }
}

impl fmt::Display for DiceExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for t in &self.terms {
            if t.negative {
                write!(f, "-")?;
            } else if !first {
                write!(f, "+")?;
            }
            write!(f, "{}d{}", t.count, t.sides)?;
            first = false;
        }
        if self.modifier != 0 || first {
            if self.modifier >= 0 && !first {
                write!(f, "+")?;
            }
            write!(f, "{}", self.modifier)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TermRoll {
    pub term: DiceTerm,
    pub rolls: Vec<u32>,
}

/// Full record of a roll: every individual die is kept for the session log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiceRoll {
    pub expr: DiceExpr,
    pub critical: bool,
    pub terms: Vec<TermRoll>,
    pub total: i32,
}

impl DiceRoll {
    /// Human-readable breakdown, e.g. `2d6+3: [4, 2] +3 = 9`.
    pub fn breakdown(&self) -> String {
        let mut s = format!(
            "{}{}: ",
            self.expr,
            if self.critical { " (crit)" } else { "" }
        );
        for (i, t) in self.terms.iter().enumerate() {
            if i > 0 || t.term.negative {
                s.push_str(if t.term.negative { " -" } else { " +" });
            }
            s.push_str(&format!("{:?}", t.rolls));
        }
        if self.expr.modifier != 0 {
            s.push_str(&format!(" {:+}", self.expr.modifier));
        }
        s.push_str(&format!(" = {}", self.total));
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::{GameRng, ScriptedRng};

    #[test]
    fn parses_common_expressions() {
        assert_eq!(
            DiceExpr::parse("1d20+5").unwrap(),
            DiceExpr::dice(1, 20).with_modifier(5)
        );
        assert_eq!(
            DiceExpr::parse("2d6+3").unwrap(),
            DiceExpr::dice(2, 6).with_modifier(3)
        );
        assert_eq!(DiceExpr::parse("4d8").unwrap(), DiceExpr::dice(4, 8));
        assert_eq!(DiceExpr::parse("d100").unwrap(), DiceExpr::dice(1, 100));
        assert_eq!(
            DiceExpr::parse(" 2D6 + 1d4 - 1 ").unwrap().to_string(),
            "2d6+1d4-1"
        );
        assert_eq!(DiceExpr::parse("5").unwrap().to_string(), "5");
        assert_eq!(DiceExpr::parse("-1d4").unwrap().to_string(), "-1d4");
    }

    #[test]
    fn rejects_bad_or_abusive_input() {
        for bad in [
            "", "d", "2d", "1d0", "0d6", "1d20++5", "abc", "1d20+x", "101d6", "1d1001", "1d6 1d6",
        ] {
            assert!(DiceExpr::parse(bad).is_err(), "should reject {bad:?}");
        }
        assert_eq!(
            DiceExpr::parse(&"1".repeat(100)),
            Err(DiceParseError::TooLong)
        );
    }

    #[test]
    fn roll_totals_include_modifiers_and_negatives() {
        let e = DiceExpr::parse("2d6+1d4-1").unwrap();
        let r = e.roll(&mut ScriptedRng::new([3, 5, 2]), false);
        assert_eq!(r.total, 3 + 5 + 2 - 1);
        let e = DiceExpr::parse("1d8-1d4").unwrap();
        assert_eq!(e.roll(&mut ScriptedRng::new([2, 4]), false).total, -2);
    }

    #[test]
    fn critical_doubles_dice_not_modifier() {
        // SRD: "if you score a Critical Hit with a Dagger, roll 2d4 for the damage rather than 1d4,
        // and add your relevant ability modifier."
        let dagger = DiceExpr::dice(1, 4).with_modifier(3);
        let r = dagger.roll(&mut ScriptedRng::new([4, 2]), true);
        assert_eq!(r.terms[0].rolls, vec![4, 2]);
        assert_eq!(r.total, 9);
    }

    #[test]
    fn rolls_stay_within_bounds() {
        let mut rng = GameRng::seeded(1);
        let e = DiceExpr::parse("3d6+2").unwrap();
        for _ in 0..1000 {
            let t = e.roll(&mut rng, false).total;
            assert!((5..=20).contains(&t));
        }
        assert_eq!(e.average(), 12.5);
    }
}
