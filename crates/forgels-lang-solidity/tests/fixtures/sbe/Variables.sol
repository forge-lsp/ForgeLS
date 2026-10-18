pragma solidity ^0.8.24;

contract Variables {
    uint public stored = 123;
    string public text = "abc";
    address public owner;

    constructor() {
        owner = msg.sender;
    }

    function doSomething() public returns (uint, string memory, address) {
        uint i = 456;
        string memory foo = "def";
        address sender = msg.sender;
        uint stamp = block.timestamp;
        stored = i;
        return i;
    }
}
