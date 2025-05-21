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

    constructor(
        address pool,
        bool zeroForOne,
        int24 currentTick,
        uint16 numTicks,
        int24 tickSpacing
    ) {
        (TickData[] memory ticks, uint64 counter) = tick_constructor(
            pool,
            zeroForOne,
            currentTick,
            numTicks,
            tickSpacing
        );

        bytes memory data = abi.encode(ticks, counter);
        assembly {
            let dataStart := add(data, 0x20)
            return(dataStart, sub(msize(), dataStart))
        }
    }

    struct TickData {
        bool initialized;
        int24 tick;
        int128 liquidityNet;
    }

    function tick_constructor(
        address pool,
        bool zeroForOne,
        int24 currentTick,
        uint16 numTicks,
        int24 tickSpacing
    ) public view returns (TickData[] memory, uint64) {
        int24 tickRange = int24(int16(numTicks)) * tickSpacing;
        int24 boundaryTick = zeroForOne
            ? currentTick + tickRange
            : currentTick - tickRange;
        if (boundaryTick < _MIN_TICK) {
            boundaryTick = _MIN_TICK;
        }
        if (boundaryTick > _MAX_TICK) {
            boundaryTick = _MAX_TICK;
        }

        int24[] memory initTicks = new int24[](
            uint256(int256((boundaryTick - currentTick + 1) / tickSpacing)) + 1
        );
        TickData[] memory ticks;

        {
            uint256 counter = 0;
            (int16 pos, int16 endPos) = (
                int16((currentTick / tickSpacing) >> 8),
                int16((boundaryTick / tickSpacing) >> 8)
            );
            if (zeroForOne) {
                for (; pos <= endPos; pos++) {
                    uint256 bm = IUniswapV3Pool(pool).tickBitmap(pos);
                    while (bm != 0) {
                        uint8 bit = _leastSignificantBit(bm);
                        bm ^= 1 << bit;
                        int24 extractedTick = ((int24(pos) << 8) |
                            int24(uint24(bit))) * int24(tickSpacing);
                        if (
                            extractedTick >= currentTick &&
                            extractedTick <= boundaryTick
                        ) {
                            initTicks[counter++] = extractedTick;
                        }
                        if (counter == numTicks) {
                            break;
                        }
                    }
                }
            } else {
                for (; pos >= endPos; pos--) {
                    uint256 bm = IUniswapV3Pool(pool).tickBitmap(pos);
                    while (bm != 0) {
                        uint8 bit = _leastSignificantBit(bm);
                        bm ^= 1 << bit;
                        int24 extractedTick = ((int24(pos) << 8) |
                            int24(uint24(bit))) * int24(tickSpacing);
                        if (
                            extractedTick >= boundaryTick &&
                            extractedTick <= currentTick
                        ) {
                            initTicks[counter++] = extractedTick;
                        }
                        if (counter == numTicks) {
                            break;
                        }
                    }
                }
            }
            ticks = new TickData[](counter);
        }

        for (uint256 i = 0; i < ticks.length; i++) {
            (bool success, bytes memory outputs) = pool.staticcall(
                abi.encodeWithSelector(0xf30dba93, initTicks[i])
            ); // ticks(int24 tick)
            if (!success) {
                continue;
            }

            int128 liquidityNet;

            assembly ("memory-safe") {
                let data := add(outputs, 0x20)
                liquidityNet := mload(add(data, 0x20))
            }

            ticks[i].liquidityNet = liquidityNet;
            ticks[i].tick = initTicks[i];
            ticks[i].initialized = true;
        }
        return (ticks, uint64(ticks.length));
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
