pragma solidity ^0.8.24;

contract ABIEncode {
    function encode(uint x, address a) public pure returns (bytes memory) {
        bytes memory data = abi.encode(x, a);
        return data;
    }

    function decode(bytes memory data) public pure returns (uint, address) {
        (uint x, address a) = abi.decode(data, (uint, address));
        return (x, a);
    }
}
