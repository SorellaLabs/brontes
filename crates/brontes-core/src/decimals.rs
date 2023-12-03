use alloy_providers::provider::Provider;
use alloy_sol_macro::sol;
use alloy_transport_http::Http;

sol!(
    function decimals() public view returns (uint8);
);
