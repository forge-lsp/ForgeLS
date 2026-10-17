pragma solidity ^0.8.24;

contract ABIDecode {
    function decode(bytes memory data) public pure returns (uint x, address a) {
        uint kept = 1;
        (x, a) = abi.decode(data, (uint, address));
        kept = x;
    }
}
