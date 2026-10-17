pragma solidity ^0.8.24;

contract Constructor {
    address public owner;
    uint public x;
    string public label;

    constructor(uint _x, string memory _label) {
        owner = msg.sender;
        x = _x;
        label = _label;
    }

    function getX() public view returns (uint) {
        return x;
    }
}
