pragma solidity ^0.8.24;

contract Vault {
    mapping(address => uint) public balances;

    event Paid(address to);

    function pay(address to) public {
        uint kept = gasleft();
        address receiver = to;
        to.call{value: 1}("");
    }
}
