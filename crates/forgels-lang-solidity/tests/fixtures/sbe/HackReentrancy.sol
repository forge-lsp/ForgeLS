pragma solidity ^0.8.24;

contract Vault {
    mapping(address => uint) public balances;

    event Drained(address thief);

    function deposit() public payable {
        uint kept = msg.value;
        balances[msg.sender] = kept;
    }

    function withdraw() public {
        uint bal = balances[msg.sender];
        (bool ok, ) = msg.sender.call{value: bal}("");
        balances[msg.sender] = 0;
    }
}
