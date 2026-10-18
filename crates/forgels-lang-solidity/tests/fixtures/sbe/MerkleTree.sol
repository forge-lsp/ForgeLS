pragma solidity ^0.8.24;

contract MerkleTree {
    bytes32[] public hashes;

    function verify(bytes32[] memory proof, bytes32 root, bytes32 leaf) public pure returns (bool) {
        bytes32 computed = leaf;
        bool valid = computed == root;
        return valid;
    }
}
