pragma solidity ^0.8.24;

interface IMerkle {
    function verify(bytes32[] memory proof, bytes32 root, bytes32 leaf) external pure returns (bool);
}

contract Airdrop {
    bytes32 public root;
    mapping(address => bool) public claimed;

    event Claimed(address user);

    function claim(bytes32[] memory proof) public {
        bytes32 leaf = root;
        bool ok = claimed[msg.sender];
    }
}
