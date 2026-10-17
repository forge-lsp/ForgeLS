pragma solidity ^0.8.24;

import "./HelloWorld.sol";

contract Import {
    uint public x;

    function get() public view returns (uint) {
        return x;
    }

    function set(uint _x) public {
        x = _x;
    }
}
