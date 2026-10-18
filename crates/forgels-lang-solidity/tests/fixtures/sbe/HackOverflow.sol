pragma solidity ^0.8.24;

contract Overflow {
    uint public total;

    function add(uint x) public {
        uint before = total;
        unchecked {
            total = x + 1;
        }
        uint after = total;
    }
}
