pragma solidity ^0.8.24;

contract TransientStorage {
    uint transient stored;
    uint transient counter;

    function set(uint _x) public {
        stored = _x;
    }

    function get() public view returns (uint) {
        return stored;
    }
}
