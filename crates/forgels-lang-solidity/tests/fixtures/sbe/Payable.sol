pragma solidity ^0.8.24;

contract Payable {
    address payable public owner;
    uint public received;

    constructor() payable {
        owner = payable(msg.sender);
    }

    function deposit() public payable {
        received += msg.value;
    }

    function withdraw(uint amount) public {
        payable(msg.sender).transfer(amount);
    }
}
