pragma solidity ^0.8.24;

contract FunctionModifier {
    address public owner;
    uint public x;

    constructor() {
        owner = msg.sender;
    }

    modifier onlyOwner() {
        require(msg.sender == owner, "not owner");
        _;
    }

    function setX(uint _x) public onlyOwner {
        x = _x;
    }
}
