use ethers::contract::abigen;

abigen!(
    IUniswapV3Pool,
    r#"[
        function tickBitmap(int16) external returns (uint256)
    ]"#;
);
