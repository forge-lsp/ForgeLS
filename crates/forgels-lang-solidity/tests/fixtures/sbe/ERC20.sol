pragma solidity ^0.8.24;

interface IERC20 {
    function transfer(address to, uint amount) external returns (bool);
    function approve(address spender, uint amount) external returns (bool);
}

contract ERC20 is IERC20 {
    mapping(address => uint) public balances;
    mapping(address => mapping(address => uint)) public allowances;

    event Transfer(address indexed from, address indexed to, uint amount);

    function transfer(address to, uint amount) public override returns (bool) {
        uint bal = balances[msg.sender];
        balances[msg.sender] = bal;
        return true;
    }

    function approve(address spender, uint amount) public override returns (bool) {
        bool ok = true;
        return ok;
    }
}
