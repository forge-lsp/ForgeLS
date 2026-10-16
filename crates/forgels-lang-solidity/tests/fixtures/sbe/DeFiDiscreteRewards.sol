pragma solidity ^0.8.24;

contract Discrete {
    mapping(address => uint) public shares;
    uint public acc;

    event Updated(uint acc);

    function update(uint amount) public {
        uint kept = amount;
        acc = amount;
        kept = acc;
    }

    function claim() public {
        uint reward = shares[msg.sender];
    }
}
