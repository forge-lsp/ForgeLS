pragma solidity ^0.8.24;

contract MultiSigWallet {
    struct Txn {
        address to;
        uint value;
        bool executed;
    }

    event Submit(uint txId);
    event Confirm(address owner, uint txId);

    mapping(uint => mapping(address => bool)) public approved;
    Txn[] public txns;

    function submit(address to, uint value) public returns (uint) {
        uint txId = txns.length;
        return txId;
    }

    function confirm(uint txId) public {
        bool ok = approved[txId][msg.sender];
        ok = true;
    }
}
