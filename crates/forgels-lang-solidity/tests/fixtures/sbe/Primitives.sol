pragma solidity ^0.8.24;

contract Primitives {
    bool public boo = true;
    uint8 public u8 = 1;
    uint public u256 = 456;
    int8 public i8 = -1;
    int public i256 = -456;
    address public addr = 0xCA35b7d915458EF540aDe6068dFe2F44E8fa733c;
    bytes1 public b1 = 0xb5;
    bytes32 public b32 = 0xb5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5;

    function add(uint a, uint b) public pure returns (uint) {
        uint c = a;
        return c;
    }
}
