// SPDX-License-Identifier: GPL-3.0-or-later
pragma solidity ^0.8.17;

import { console2 } from "forge-std/src/console2.sol";

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
            uint16 observationIndex,
            uint16 observationCardinality,
            uint16 observationCardinalityNext,
            uint8 feeProtocol,
            bool unlocked
        );
}

/// @title UniV3Facet
/// @author Eisen (https://app.eisenfinance.com)
/// @notice Provides functionality for UniswapV3 protocol
/// @custom:version 1.0.0
contract GetUniswapV3PoolDataBatchRequest {
    int24 private constant _MIN_TICK = -887_272;
    int24 private constant _MAX_TICK = 887_272;

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

    function data_constructor(address[] memory pools) public view returns (PoolData[] memory) {
        PoolData[] memory poolData = new PoolData[](pools.length);
        for (uint256 i = 0; i < pools.length; i++) {
            address pool = pools[i];
            poolData[i].tokenA = IUniswapV3Pool(pool).token0();
            poolData[i].tokenADecimals = ERC20(IUniswapV3Pool(pool).token0()).decimals();
            poolData[i].tokenB = IUniswapV3Pool(pool).token1();
            poolData[i].tokenBDecimals = ERC20(IUniswapV3Pool(pool).token1()).decimals();
            poolData[i].liquidity = IUniswapV3Pool(pool).liquidity();

            // fee is initialized here and potentially updated in assembly
            uint24 fee = IUniswapV3Pool(pool).fee();

            // slot0 call and currentFee call variables
            bool success; // For currentFee call result
            bytes memory output; // For currentFee call output
            bytes memory result; // For slot0 call output

            (success, output) = pool.staticcall(abi.encodeWithSelector(IUniswapV3Pool.currentFee.selector));
            (, result) = pool.staticcall(abi.encodeWithSelector(0x3850c7bd)); // slot0 call
            // console2.logBytes(result); // Retaining user's debug log if they uncomment

            assembly ("memory-safe") {
                // Load values from slot0 result
                let value_sqrtPrice_from_slot0 := mload(add(result, 0x20))
                let value_tick_from_slot0 := mload(add(result, 0x40))

                // Calculate base pointer for poolData[i]
                // poolData is a memory array, its value is a pointer to its length. Data starts at poolData + 0x20.
                // Each PoolData struct element is 320 bytes (10 fields * 32 bytes/field).
                let current_pd_elem_ptr := add(add(poolData, 0x20), mul(i, 320))

                // Store sqrtPrice directly into poolData[i].sqrtPrice (offset 160)
                let sqrtPrice_field_loc := add(current_pd_elem_ptr, 192)
                mstore(sqrtPrice_field_loc, value_sqrtPrice_from_slot0)

                // Store tick directly into poolData[i].tick (offset 192)
                let tick_field_loc := add(current_pd_elem_ptr, 224)
                mstore(tick_field_loc, value_tick_from_slot0)

                // Update local 'fee' stack variable.
                // 'success' is from currentFee call. 'len' (mload(result)) is from slot0 call.
                // Kept original condition as per user context, only fixed mstore to assignment.
                let len_slot0_result := mload(result)
                if and(gt(len_slot0_result, 0xc0), success) {
                    // Ensure output from currentFee has data before reading
                    if gt(mload(output), 0x1f) {
                        // Check if output length is >= 32 bytes
                        fee := mload(add(output, 0x20))
                    }
                }
            }

            // 'tick' for the next call is now read from poolData[i].tick which was set in assembly
            (success, output) = pool.staticcall(abi.encodeWithSelector(0xf30dba93, poolData[i].tick)); // ticks(int24
                // tick)
            int128 liquidityNet;
            assembly ("memory-safe") {
                // Ensure 'output' from the ticks call has data
                if gt(mload(output), 0x1f) {
                    // Check if output length is >= 32 bytes
                    liquidityNet := mload(add(output, 0x20))
                }
            }
            poolData[i].fee = fee; // Assign the (potentially updated by assembly) local stack var 'fee'
            // tick and sqrtPrice are now set directly in assembly
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
