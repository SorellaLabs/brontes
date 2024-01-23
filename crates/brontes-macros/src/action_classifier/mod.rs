mod action_dispatch;
mod action_impl;
mod call_data;
mod closure_dispatch;
mod data_preparation;
mod logs;
mod return_data;

pub use action_dispatch::ActionDispatch;
pub use action_impl::ActionMacro;

/// used to link the action_sig from the action macro
/// to the action dispatch macro;
/// the ac
/// format!("{ACTION_SIG_NAME}_{action_struct_name}",
pub(super) const ACTION_SIG_NAME: &str = "__action_sig";
