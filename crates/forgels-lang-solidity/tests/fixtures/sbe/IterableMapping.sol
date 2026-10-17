pragma solidity ^0.8.24;

contract IterableMapping {
    mapping(address => uint) public balances;
    address[] public keys;

    function set(address user, uint amount) public {
        uint idx = balances[user];
        balances[user] = amount;
        idx = amount;
    }

    function get(address user) public view returns (uint) {
        uint value = balances[user];
        return value;
    }
}
