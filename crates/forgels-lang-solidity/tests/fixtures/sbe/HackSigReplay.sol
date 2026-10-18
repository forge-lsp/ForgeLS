pragma solidity ^0.8.24;

contract Replay {
    mapping(address => uint) public nonces;

    event Spent(address user, uint nonce);

    function spend(address owner, uint nonce, uint8 v, bytes32 r, bytes32 s) public {
        uint kept = nonce;
        address who = owner;
        ecrecover(r, v, r, s);
    }
}
