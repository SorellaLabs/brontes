use std::{
    collections::HashMap,
    fmt,
    pin::Pin,
    string::ToString,
    task::{ready, Context, Poll},
};

use futures::Future;
use serde::{
    de::{self, Deserializer, Visitor},
    Deserialize, Serialize,
};
use strum::Display;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::mev::{Bundle, MevBlock};

/// metric event for traces
#[derive(Debug, Clone, Serialize, Display, Deserialize)]
pub enum TuiEvents {
    MevBlockMetricReceived(MevBlock),
    MevBundleEventReceived(Vec<Bundle>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Display, Deserialize)]
pub enum Action {
    Tick,
    Render,
    Resize(u16, u16),
    Suspend,
    Resume,
    Quit,
    Refresh,
    Tui(TuiEvents),
    Error(String),
    Help,
}

impl PartialEq for TuiEvents {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (TuiEvents::MevBlockMetricReceived(a), TuiEvents::MevBlockMetricReceived(b)) => a == b,
            (TuiEvents::MevBundleEventReceived(a), TuiEvents::MevBundleEventReceived(b)) => {
                // Custom logic to compare Vec<Bundle>
                // For example, if Bundle implements PartialEq and order doesn't matter:
                a.len() == b.len() && a.iter().all(|item| b.contains(item))
            }
            _ => false,
        }
    }
}

impl Eq for TuiEvents {}
