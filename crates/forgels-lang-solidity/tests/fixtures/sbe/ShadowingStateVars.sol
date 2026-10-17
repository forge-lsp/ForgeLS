pragma solidity ^0.8.24;

contract X {
    string public name = "X";
    uint public value = 1;

    function getName() public view returns (string memory) {
        return name;
    }
}

contract Y is X {
    string public name = "Y";

    function getY() public view returns (string memory) {
        return name;
    }
}
