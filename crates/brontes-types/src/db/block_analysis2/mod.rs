mod cex_dex;

trait BlockAnalysisSerde {
    fn serialize_into<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error>;

    fn deserialize_into<'de, D>(deserializer: D) -> Result<Self, D::Error>
    where
        Self: Sized,
        D: serde::Deserializer<'de>;
}
