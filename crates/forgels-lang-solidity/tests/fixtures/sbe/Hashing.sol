pragma solidity ^0.8.24;

contract Hashing {
    function hash(string memory text, uint num, address addr) public pure returns (bytes32) {
        bytes32 digest = keccak256(abi.encodePacked(text, num, addr));
        return digest;
    }
}
