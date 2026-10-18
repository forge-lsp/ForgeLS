pragma solidity ^0.8.24;

interface Counter {
    function count() external view returns (uint);
    function inc() external;
}

contract MyCounter is Counter {
    uint public count;

    function inc() public override {
        count += 1;
    }
}
