use std::collections::HashSet;

use alloy_primitives::BlockHash;
use relays_openapi::apis::{
    configuration::Configuration,
    data_api::{get_delivered_payloads, get_received_bids},
};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use strum::IntoEnumIterator;

use super::RelayBlockMetadata;
use crate::block_metadata::{RelayBid, RelayPayload};

macro_rules! relays {
    ($([$relay:ident, $min_block:literal, $url:expr]),*) => {

        #[derive(
            Debug, Copy, Clone, serde::Serialize, serde::Deserialize, Eq, PartialEq,
            std::hash::Hash, PartialOrd, strum::EnumIter
        )]
        pub enum Relays {
            $($relay),*
        }

        impl Relays {
            pub fn min_block_with_data(&self) -> u64 {
                match self {
                    $(
                        Relays::$relay => $min_block,
                    )*
                }
            }

            pub fn url(&self) -> &str {
                match self {
                    $(
                        Relays::$relay => $url,
                    )*
                }
            }
        }

        impl std::fmt::Display for Relays {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{:?}", self)
            }
        }
    };
}

relays!(
    [
        UltraSound,
        16655143,
        "https://0xa1559ace749633b997cb3fdacffb890aeebdb0f5a3b6aaa7eeeaf1a38af0a8fe88b9e4b1f61f236d2e64d95733327a62@relay.ultrasound.money"
    ],
    [
        Titan,
        15885217,
        "https://0x8c4ed5e24fe5c6ae21018437bde147693f68cda427cd1122cf20819c30eda7ed74f72dece09bb313f2a1855595ab677d@global.titanrelay.xyz"

    ],
    [
        AgnosticGnosis,
        16691069,
        "https://0xa7ab7a996c8584251c8f925da3170bdfd6ebc75d50f5ddc4050a6fdc77f2a3b5fce2cc750d0865e05d7228af97d69561@agnostic-relay.net"
    ],
    [
        BloxrouteMaxProfit,
        16891314,
        "https://0x8b5d2e73e2a3a55c6c87b8b6eb92e0149a125c852751db1422fa951e42a09b82c142c3ea98d0d9930b056a3bc9896b8f@bloxroute.max-profit.blxrbdn.com"
    ],
    [
        BloxrouteRegulated,
        16954266,
        "https://0xb0b07cd0abef743db4260b0ed50619cf6ad4d82064cb4fbec9d3ec530f7c5e6793d9f286c4e082c0244ffb9f2658fe88@bloxroute.regulated.blxrbdn.com"
    ],
    [
        Flashbots,
        15750649,
        "https://0xac6e77dfe25ecd6110b8e780608cce0dab71fdd5ebea22a16c0205200f2f8e2e3ad3b71d3499c54ad14d6c21b41a37ae@boost-relay.flashbots.net"
    ],
    [
        Aestus,
        17346135,
        "https://0xa15b52576bcbf1072f4a011c0f99f9fb6c66f3e1ff321f11f461d15e31b1cb359caa092c71bbded0bae5b5ea401aab7e@aestus.live"
    ]
);

impl Relays {
    fn configuration(&self) -> Configuration {
        Configuration { base_path: self.url().to_string(), ..Default::default() }
    }

    pub async fn get_relay_metadata(
        block_number: u64,
        block_hash: BlockHash,
    ) -> eyre::Result<Option<RelayBlockMetadata>> {
        let block_hash = format!("{:?}", block_hash);
        let bids = futures::future::join_all(Relays::iter().map(|relay| {
            let block_hash = block_hash.clone();

            async move {
                match relay.get_winning_bid(block_number, block_hash).await {
                    Ok(r) => Some(r),
                    Err(e) => {
                        tracing::warn!(%relay, "error getting bids - {:?}", e);
                        None
                    }
                }
            }
        }))
        .await
        .into_iter()
        .flatten()
        .flatten()
        .collect::<Vec<_>>();

        if let Some(best_bid) = bids
            .into_iter()
            .min_by(|a, b| a.timestamp_ms.cmp(&b.timestamp_ms))
        {
            Ok(Some(best_bid.try_into()?))
        } else {
            /*
            try's to the get the ultrasound relay payload as their bid might not have the right hash.
             */
            let relay = Relays::UltraSound;
            match relay.get_payload(block_number).await {
                Ok(r) => Ok(r.map(TryInto::try_into).transpose()?),
                Err(e) => {
                    tracing::warn!(%relay, "error getting payloads - {:?}", e);
                    Ok(None)
                }
            }
        }
    }

    async fn get_winning_bid(
        self,
        block_number: u64,
        block_hash: String,
    ) -> eyre::Result<Option<RelayBid>> {
        let mut bids = self
            .get_received_bids(None, None, Some(block_number.to_string()), None, None)
            .await?;

        let find_winning_bid = |bids: Vec<RelayBid>| {
            bids.into_iter()
                .filter(|bid| bid.block_hash.to_lowercase() == block_hash.to_lowercase())
                .min_by(|a, b| a.timestamp_ms.cmp(&b.timestamp_ms))
        };

        if let Some(winning_bid) = find_winning_bid(bids.clone()) {
            Ok(Some(winning_bid))
        } else if matches!(self, Relays::UltraSound) {
            let slots = bids.iter().map(|bid| bid.slot).collect::<HashSet<_>>();
            let adj_bids = get_ultrasound_adj(slots).await?;

            bids.iter_mut().for_each(|bid| {
                adj_bids.iter().for_each(|adj_bid| {
                    if adj_bid.submitted_block_hash == bid.block_hash
                        && adj_bid.submitted_value == bid.value
                        && adj_bid.builder_pubkey == bid.builder_pubkey
                    {
                        bid.block_hash = adj_bid.adjusted_block_hash.clone();
                        bid.value = adj_bid.adjusted_value;
                    }
                })
            });

            Ok(find_winning_bid(bids))
        } else {
            Ok(None)
        }
    }

    async fn get_payload(self, block_number: u64) -> eyre::Result<Option<RelayPayload>> {
        let payloads = self
            .get_delivered_payloads(
                None,
                None,
                None,
                None,
                Some(block_number.to_string()),
                None,
                None,
                None,
            )
            .await?;

        Ok(payloads.first().cloned())
    }

    async fn get_received_bids(
        &self,
        slot: Option<String>,
        block_hash: Option<String>,
        block_number: Option<String>,
        builder_pubkey: Option<String>,
        limit: Option<String>,
    ) -> eyre::Result<Vec<RelayBid>> {
        let bids = get_received_bids(
            &self.configuration(),
            slot.as_deref(),
            block_hash.as_deref(),
            block_number.as_deref(),
            builder_pubkey.as_deref(),
            limit.as_deref(),
        )
        .await?;

        Ok(bids
            .into_iter()
            .map(|bid| RelayBid::new(bid, *self))
            .collect())
    }

    async fn get_delivered_payloads(
        &self,
        slot: Option<String>,
        cursor: Option<String>,
        limit: Option<String>,
        block_hash: Option<String>,
        block_number: Option<String>,
        proposer_pubkey: Option<String>,
        builder_pubkey: Option<String>,
        order_by: Option<String>,
    ) -> eyre::Result<Vec<RelayPayload>> {
        let payloads = get_delivered_payloads(
            &self.configuration(),
            slot.as_deref(),
            cursor.as_deref(),
            limit.as_deref(),
            block_hash.as_deref(),
            block_number.as_deref(),
            proposer_pubkey.as_deref(),
            builder_pubkey.as_deref(),
            order_by.as_deref(),
        )
        .await?;

        Ok(payloads
            .into_iter()
            .map(|payload| RelayPayload::new(payload, *self))
            .collect())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UltrasoundAdjBidResponse {
    pub data: Vec<UltrasoundAdjBid>,
}

#[serde_as]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UltrasoundAdjBid {
    pub adjusted_block_hash: String,
    #[serde_as(as = "DisplayFromStr")]
    pub adjusted_value: u128,
    pub block_number: u64,
    pub builder_pubkey: String,
    #[serde_as(as = "DisplayFromStr")]
    pub delta: u128,
    pub submitted_block_hash: String,
    pub submitted_received_at: String,
    #[serde_as(as = "DisplayFromStr")]
    pub submitted_value: u128,
}

async fn get_ultrasound_adj(slots: HashSet<u64>) -> eyre::Result<Vec<UltrasoundAdjBid>> {
    let client = &reqwest::Client::new();

    let adjusted = futures::future::join_all(slots.into_iter().map(|slot| async move {
        let url = format!(
            "https://relay-analytics.ultrasound.money/ultrasound/v1/data/adjustments?slot={slot}"
        );

        let bid: UltrasoundAdjBidResponse = client.get(url).send().await?.json().await?;
        Ok::<_, eyre::ErrReport>(bid.data)
    }))
    .await
    .into_iter()
    .collect::<Result<Vec<_>, _>>()?;

    Ok(adjusted.into_iter().flatten().collect())
}
