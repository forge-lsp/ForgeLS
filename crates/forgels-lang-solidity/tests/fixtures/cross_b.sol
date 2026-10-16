pragma solidity ^0.8.0;

contract Vault {
    uint256 total;

    function deposit(uint amount) public {
        total = amount;
    }

    function withdraw(uint amount) public returns (uint) {
        return amount;
    }
}
