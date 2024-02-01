use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
};

use alloy_primitives::Address;
use serde::{Deserialize, Serialize};

use crate::Protocol;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SubGraphsEntry(pub HashMap<u64, Vec<SubGraphEdge>>);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubGraphEdge {
    pub info:                   PoolPairInfoDirection,
    pub distance_to_start_node: u8,
    pub distance_to_end_node:   u8,
}
impl Deref for SubGraphEdge {
    type Target = PoolPairInfoDirection;

    fn deref(&self) -> &Self::Target {
        &self.info
    }
}
impl DerefMut for SubGraphEdge {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.info
    }
}

impl SubGraphEdge {
    pub fn new(
        info: PoolPairInfoDirection,
        distance_to_start_node: u8,
        distance_to_end_node: u8,
    ) -> Self {
        Self { info, distance_to_end_node, distance_to_start_node }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PoolPairInformation {
    pub pool_addr: Address,
    pub dex_type:  Protocol,
    pub token_0:   Address,
    pub token_1:   Address,
}

impl PoolPairInformation {
    pub fn new(pool_addr: Address, dex_type: Protocol, token_0: Address, token_1: Address) -> Self {
        Self { pool_addr, dex_type, token_0, token_1 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PoolPairInfoDirection {
    pub info:       PoolPairInformation,
    pub token_0_in: bool,
}

impl PoolPairInfoDirection {
    pub fn new(info: PoolPairInformation, token_0_in: bool) -> Self {
        Self { info, token_0_in }
    }
}

impl Deref for PoolPairInfoDirection {
    type Target = PoolPairInformation;

    fn deref(&self) -> &Self::Target {
        &self.info
    }
}

impl DerefMut for PoolPairInfoDirection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.info
    }
}

impl PoolPairInfoDirection {
    pub fn get_base_token(&self) -> Address {
        if self.token_0_in {
            self.info.token_0
        } else {
            self.info.token_1
        }
    }

    pub fn get_quote_token(&self) -> Address {
        if self.token_0_in {
            self.info.token_1
        } else {
            self.info.token_0
        }
    }
}
