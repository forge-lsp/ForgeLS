pragma solidity ^0.8.24;

contract EtherWallet {
    address payable public owner;

    constructor() payable {
        owner = payable(msg.sender);
    }

    receive() external payable { }

    function withdraw(uint amount) public {
        uint bal = address(this).balance;
        payable(msg.sender).transfer(amount);
        bal = amount;
    }
}
