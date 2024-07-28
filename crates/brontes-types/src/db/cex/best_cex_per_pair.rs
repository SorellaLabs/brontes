/// stores the best cex (most volume on the pair).
/// this is used to choose what cex is most likely the
/// driver of true price
#[derive(Debug, Clone, Row, Serialize, Deserialize)]
pub struct BestCexPerPair {
    pub a,
}
