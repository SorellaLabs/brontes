pub mod ser_mev_block {

    use serde::ser::{Serialize, SerializeStruct};
    use sorella_db_databases::clickhouse::fixed_string::FixedString;

    use crate::classified_mev::MevBlock;

    impl Serialize for MevBlock {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            let mut ser_struct = serializer.serialize_struct("MevBlock", 15)?;

            ser_struct.serialize_field(
                "block_hash",
                &FixedString::from(format!("{:?}", self.block_hash)),
            )?;
            ser_struct.serialize_field("block_number", &self.block_number)?;
            ser_struct.serialize_field("mev_count", &self.mev_count)?;
            ser_struct.serialize_field("finalized_eth_price", &self.finalized_eth_price)?;
            ser_struct.serialize_field("cumulative_gas_used", &self.cumulative_gas_used)?;
            ser_struct.serialize_field("cumulative_gas_paid", &self.cumulative_gas_paid)?;
            ser_struct.serialize_field("total_bribe", &self.total_bribe)?;
            ser_struct.serialize_field(
                "cumulative_mev_priority_fee_paid",
                &self.cumulative_mev_priority_fee_paid,
            )?;
            ser_struct.serialize_field(
                "builder_address",
                &FixedString::from(format!("{:?}", self.builder_address)),
            )?;
            ser_struct.serialize_field("builder_eth_profit", &self.builder_eth_profit)?;
            ser_struct.serialize_field(
                "builder_finalized_profit_usd",
                &self.builder_finalized_profit_usd,
            )?;

            ser_struct.serialize_field(
                "proposer_fee_recipient",
                &self
                    .proposer_fee_recipient
                    .map(|addr| FixedString::from(format!("{:?}", addr))),
            )?;
            ser_struct.serialize_field("proposer_mev_reward", &self.proposer_mev_reward)?;
            ser_struct.serialize_field(
                "proposer_finalized_profit_usd",
                &self.proposer_finalized_profit_usd,
            )?;
            ser_struct.serialize_field(
                "cumulative_mev_finalized_profit_usd",
                &self.cumulative_mev_finalized_profit_usd,
            )?;

            ser_struct.end()
        }
    }
}
