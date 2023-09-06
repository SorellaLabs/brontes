use crate::normalize::Structure;

/// Represents the amount of MEV extracted for a particular type of strategy.
pub struct Report;

pub trait Inspector {
    fn inspect(&self, inspection: Vec<Structure>) -> Report;
}
