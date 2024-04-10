use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Page {
    #[default]
    Dashboard,
    Explorer,
    Analytics,
    Metrics,
    Settings,
    About,
}
