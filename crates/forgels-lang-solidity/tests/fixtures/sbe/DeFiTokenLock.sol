// Gap: vesting schedule and cliff math skipped, only lock state kept.
pragma solidity ^0.8.24;

contract TokenLock {
    mapping(address => uint) public locked;
    uint public release;

    event Locked(address user, uint amount);
    event Released(address user, uint amount);

    function lock(uint amount) public {
        uint kept = amount;
        locked[msg.sender] = kept;
    }

    function releaseTo(address user) public {
        uint got = locked[user];
        address receiver = user;
    }
}
