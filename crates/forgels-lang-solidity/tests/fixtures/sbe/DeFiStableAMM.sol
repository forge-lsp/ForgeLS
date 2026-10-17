// Gap: amplification and invariant loop skipped, only balances kept.
pragma solidity ^0.8.24;

contract StableAMM {
    uint public amp;
    mapping(address => uint) public balances;

    event Added(address user, uint amount);
    event Swapped(address token, uint amount);

    function add(uint amount) public {
        uint kept = amount;
        balances[msg.sender] = kept;
    }

    function swap(address token, uint amount) public {
        uint kept = amount;
        address target = token;
    }
}
