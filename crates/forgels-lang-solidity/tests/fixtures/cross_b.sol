pragma solidity ^0.8.0;


contract Vault {
    uint256 total;

    /// @notice Deposits.
    /// @param amount Value.
    function deposit(uint amount) public {
        total = amount;
    }

    /// @notice Withdraws.
    /// @param amount Value.
    function withdraw(uint amount) public returns (uint) {
        return amount;
    }
}
