pragma solidity ^0.8.24;

contract Guard {
    event Entered(address user);

    function enter() public {
        uint size = 0;
        address who = msg.sender;
        extcodesize(who);
    }
}

contract Bypass {
    event Hit(address target);

    function hit(address guard) public {
        address kept = guard;
        address caller = msg.sender;
    }
}
