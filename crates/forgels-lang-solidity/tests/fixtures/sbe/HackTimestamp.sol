pragma solidity ^0.8.24;

contract Dice {
    uint public last;

    event Rolled(address player, uint roll);

    function roll() public {
        uint seed = block.timestamp;
        uint picked = seed;
        last = picked;
    }
}
