// SPDX-License-Identifier: GPL-3.0-or-later
pragma solidity ^0.8.17;

interface ERC20 {
    function name() external view returns (string memory);

    function symbol() external view returns (string memory);

    function decimals() external view returns (uint8);
}

contract GetERC20DataBatchRequest {
    constructor(address[] memory tokens) {
        TokenData[] memory tokenData = data_constructor(tokens);
        bytes memory abiEncodedData = abi.encode(tokenData);
        assembly {
            let dataStart := add(abiEncodedData, 0x20)
            return(dataStart, sub(msize(), dataStart))
        }
    }

    struct TokenData {
        address token;
        string name;
        string symbol;
        uint8 decimals;
    }

    /// @notice Get uniV2 pool param info using pool address
    function data_constructor(address[] memory tokens) public view returns (TokenData[] memory) {
        TokenData[] memory tokenData = new TokenData[](tokens.length);
        for (uint256 i = 0; i < tokens.length; i++) {
            address token = tokens[i];
            tokenData[i].token = token;
            tokenData[i].name = ERC20(token).name();
            tokenData[i].symbol = ERC20(token).symbol();
            tokenData[i].decimals = ERC20(token).decimals();
        }
        return tokenData;
    }
}
