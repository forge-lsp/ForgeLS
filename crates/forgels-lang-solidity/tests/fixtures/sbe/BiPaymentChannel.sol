pragma solidity ^0.8.24;

contract BiChannel {
    address public alice;
    address public bob;
    uint public nonce;

    event Settled(uint nonce);

    function settle(uint n, uint aBal, uint bBal) public {
        uint kept = n;
        bool valid = true;
    }

    function challenge() public {
        uint stamp = block.timestamp;
    }
}
