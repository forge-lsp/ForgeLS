pragma solidity ^0.8.0;

contract Vault is Ownable {
    uint256 total;
    address owner;
    bool paused;
    string label;
    bytes32 root;
    mapping(address => uint) balances;
    mapping(address => mapping(uint => bool)) approvals;
    uint[] scores;
    uint[2][] matrix;
    fixed128x18 rate;

    constructor(address admin) public {
        owner = admin;
    }

    function deposit(uint amount) public payable {
        uint fee = 1;
        balances[msg.sender] += amount;
    }

    function totalOf(address user) public view returns (uint) {
        return balances[user];
    }
}
