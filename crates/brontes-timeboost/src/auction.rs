use std::sync::Arc;

use alloy_primitives::{hex, Address, U256};
use alloy_rpc_types::Filter;
use alloy_sol_macro::sol;
use alloy_sol_types::SolEvent;
use brontes_types::traits::TracingProvider;

sol!(IExpressLaneAuction, "./src/contracts/IExpressLaneAuction.json");

pub enum ExpressLaneAuctionUpdate {
    SetExpressLaneController(ExpressLaneControllerInfo),
    AuctionResolved(ExpressLaneAuctionInfo),
}

// TODO(jinmel): Parametrize this address
pub const ONE_EXPRESS_LANE_AUCTION_ADDRESS: Address =
    Address::new(hex!("5fcb496a31b7AE91e7c9078Ec662bd7A55cd3079"));

pub struct ExpressLaneControllerInfo {
    pub round: u64,
    pub new_express_lane_controller: Address,
    pub previous_express_lane_controller: Address,
    pub transferor: Address,
    pub start_timestamp: u64,
    pub end_timestamp: u64,
}

pub struct ExpressLaneAuctionInfo {
    pub round: u64,
    pub first_price_bidder: Address,
    pub first_price_express_lane_controller: Address,
    pub first_price_amount: U256,
    pub price: U256,
    pub round_start_timestamp: u64,
    pub round_end_timestamp: u64,
}

pub struct ExpressLaneAuction<T: TracingProvider> {
    provider:         Arc<T>,
    contract_address: Address,
}

impl<T: TracingProvider> Clone for ExpressLaneAuction<T> {
    fn clone(&self) -> Self {
        Self { provider: self.provider.clone(), contract_address: self.contract_address }
    }
}

impl<T: TracingProvider> ExpressLaneAuction<T> {
    pub fn new(provider: Arc<T>) -> Self {
        Self { provider, contract_address: ONE_EXPRESS_LANE_AUCTION_ADDRESS }
    }

    pub async fn fetch_auction_events(
        &self,
        block_number: u64,
    ) -> eyre::Result<Vec<ExpressLaneAuctionUpdate>> {
        let topics = vec![
            IExpressLaneAuction::SetExpressLaneController::SIGNATURE_HASH,
            IExpressLaneAuction::AuctionResolved::SIGNATURE_HASH,
        ];

        let filter = Filter::new()
            .address(self.contract_address)
            .event_signature(topics)
            .from_block(block_number)
            .to_block(block_number);
        let logs = self.provider.get_logs(&filter).await?;

        if logs.is_empty() {
            return Ok(vec![]);
        }

        let mut updates = Vec::new();

        for log in logs {
            // Check the first topic (event signature) to determine which event type this is
            if let Some(topic0) = log.topics().first() {
                if *topic0 == IExpressLaneAuction::SetExpressLaneController::SIGNATURE_HASH {
                    // Decode as SetExpressLaneController event
                    let event = IExpressLaneAuction::SetExpressLaneController::decode_log(
                        &log.inner, true,
                    )?;
                    updates.push(ExpressLaneAuctionUpdate::SetExpressLaneController(
                        ExpressLaneControllerInfo {
                            round: event.round,
                            new_express_lane_controller: event.newExpressLaneController,
                            previous_express_lane_controller: event.previousExpressLaneController,
                            transferor: event.transferor,
                            start_timestamp: event.startTimestamp,
                            end_timestamp: event.endTimestamp,
                        },
                    ));
                } else if *topic0 == IExpressLaneAuction::AuctionResolved::SIGNATURE_HASH {
                    // Decode as AuctionResolved event
                    let event = IExpressLaneAuction::AuctionResolved::decode_log(&log.inner, true)?;
                    updates.push(ExpressLaneAuctionUpdate::AuctionResolved(
                        ExpressLaneAuctionInfo {
                            round: event.round,
                            first_price_bidder: event.firstPriceBidder,
                            first_price_express_lane_controller: event
                                .firstPriceExpressLaneController,
                            first_price_amount: event.firstPriceAmount,
                            price: event.price,
                            round_start_timestamp: event.roundStartTimestamp,
                            round_end_timestamp: event.roundEndTimestamp,
                        },
                    ));
                }
            }
        }

        Ok(updates)
    }
}
