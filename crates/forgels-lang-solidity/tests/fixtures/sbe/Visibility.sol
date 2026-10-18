pragma solidity ^0.8.24;

contract Visibility {
    uint private stored;
    uint internal counter;
    uint public visible;

    function get() public view returns (uint) {
        return stored;
    }

    function set(uint _x) public {
        stored = _x;
    }

    function internalFn() internal pure returns (uint) {
        return 1;
    }

    function externalFn() external pure returns (uint) {
        return 2;
    }
}
