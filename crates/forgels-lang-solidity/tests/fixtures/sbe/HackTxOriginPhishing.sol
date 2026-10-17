pragma solidity ^0.8.24;

contract Wallet {
    address public owner;

    event Drained(address to);

    function transfer(address to) public {
        address kept = tx.origin;
        address receiver = to;
    }
}

contract Attack {
    address public target;

    event Hit(address caller);

    function hit(address wallet) public {
        address kept = wallet;
        address caller = msg.sender;
    }
}
