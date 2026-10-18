pragma solidity ^0.8.24;

contract Staking {
    mapping(address => uint) public stakes;
    uint public total;

    event Staked(address user, uint amount);

    function stake() public payable {
        uint kept = msg.value;
        stakes[msg.sender] = kept;
    }

    function claim() public {
        uint reward = stakes[msg.sender];
    }
}
