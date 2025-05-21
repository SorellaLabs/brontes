// SPDX-License-Identifier: GPL-3.0-or-later
pragma solidity ^0.8.17;

interface ERC20 {
    function decimals() external view returns (uint8);

    function symbol() external view returns (string memory);

    function name() external view returns (string memory);

    function totalSupply() external view returns (uint256);

    function balanceOf(address account) external view returns (uint256);
}

interface IUniswapV3Pool {
    function token0() external view returns (address);

    function token1() external view returns (address);

    function tickSpacing() external view returns (int24);

    function tickBitmap(int24 tick) external view returns (uint256);

    function currentFee() external view returns (uint24);

    function liquidity() external view returns (uint128);

    function sqrtPriceX96() external view returns (uint160);

    function fee() external view returns (uint24);

    function slot0()
        external
        view
        returns (
            uint160 sqrtPriceX96,
            int24 tick,
            uint16 fee,
            uint160 unlocked,
            bool unlocked0,
            bool unlocked1
        );
}

/// @title UniV3Facet
/// @author Eisen (https://app.eisenfinance.com)
/// @notice Provides functionality for UniswapV3 protocol
/// @custom:version 1.0.0
contract GetUniswapV3TickDataBatchRequest {
    int24 private constant _MIN_TICK = -887272;
    int24 private constant _MAX_TICK = 887272;

    constructor(address[] memory pools) {
        PoolData[] memory poolData = data_constructor(pools);

        bytes memory data = abi.encode(poolData);
        assembly {
            let dataStart := add(data, 0x20)
            return(dataStart, sub(msize(), dataStart))
        }
    }

    struct PoolData {
        address tokenA;
        uint8 tokenADecimals;
        address tokenB;
        uint8 tokenBDecimals;
        uint128 liquidity;
        uint160 sqrtPrice;
        int24 tick;
        int24 tickSpacing;
        uint24 fee;
        int128 liquidityNet;
    }
    struct TickData {
        bool initialized;
        int24 tick;
        int128 liquidityNet;
    }

    /// @notice Get uniV3 pool param info using pool address

    function data_constructor(
        address[] memory pools
    ) public view returns (PoolData[] memory) {
        PoolData[] memory poolData = new PoolData[](pools.length);
        for (uint256 i = 0; i < pools.length; i++) {
            address pool = pools[i];
            poolData[i].tokenA = IUniswapV3Pool(pool).token0();
            poolData[i].tokenADecimals = ERC20(IUniswapV3Pool(pool).token0())
                .decimals();
            poolData[i].tokenB = IUniswapV3Pool(pool).token1();
            poolData[i].tokenBDecimals = ERC20(IUniswapV3Pool(pool).token1())
                .decimals();
            poolData[i].liquidity = IUniswapV3Pool(pool).liquidity();
            uint160 sqrtPriceX96;
            uint24 fee = IUniswapV3Pool(pool).fee();
            int24 tick;
            (bool success, bytes memory output) = pool.staticcall(
                abi.encodeWithSelector(IUniswapV3Pool.currentFee.selector)
            );
            (, bytes memory result) = pool.staticcall(
                abi.encodeWithSelector(0x3850c7bd)
            ); // slot0 call
            assembly ("memory-safe") {
                let len := mload(result)
                mstore(sqrtPriceX96, mload(add(result, 0x20))) // response.sqrtPriceX96
                mstore(tick, mload(add(result, 0x40))) // response.tick
                if and(gt(len, 0xc0), success) {
                    mstore(fee, mload(add(output, 0x20))) // response.feeProtocol [ramses cases]
                }
            }
            (success, output) = pool.staticcall(
                abi.encodeWithSelector(0xf30dba93, tick)
            );
            int128 liquidityNet;
            assembly ("memory-safe") {
                liquidityNet := mload(add(output, 0x20))
            }
            poolData[i].fee = fee;
            poolData[i].tick = tick;
            poolData[i].sqrtPrice = sqrtPriceX96;
            poolData[i].tickSpacing = IUniswapV3Pool(pool).tickSpacing();
            poolData[i].liquidityNet = liquidityNet;
        }
        return poolData;
    }

    /// @dev Gets the least significant bit
    function _leastSignificantBit(uint256 x) private pure returns (uint8 r) {
        require(x > 0, "x is 0");
        x = x & (~x + 1);

        if (x >= 0x100000000000000000000000000000000) {
            x >>= 128;
            r += 128;
        }
        if (x >= 0x10000000000000000) {
            x >>= 64;
            r += 64;
        }
        if (x >= 0x100000000) {
            x >>= 32;
            r += 32;
        }
        if (x >= 0x10000) {
            x >>= 16;
            r += 16;
        }
        if (x >= 0x100) {
            x >>= 8;
            r += 8;
        }
        if (x >= 0x10) {
            x >>= 4;
            r += 4;
        }
        if (x >= 0x4) {
            x >>= 2;
            r += 2;
        }
        if (x >= 0x2) r += 1;
    }
}
