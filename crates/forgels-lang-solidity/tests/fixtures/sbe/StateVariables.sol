pragma solidity ^0.8.24;

contract StateVariables {
    string public text;
    uint public num = 123;
    address public owner;

    constructor() {
        owner = msg.sender;
    }

    function set(string memory _text, uint _num) public {
        text = _text;
        num = _num;
    }

    function get() public view returns (string memory, uint, address) {
        string memory t = text;
        return (t, num, owner);
    }
}
