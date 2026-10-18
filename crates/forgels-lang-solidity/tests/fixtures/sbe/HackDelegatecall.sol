pragma solidity ^0.8.24;

contract Logic {
    uint public x;

    function set(uint v) public {
        x = v;
    }
}

contract Hack {
    address public target;

    event Pwned(address target);

    function pwn(address logic, uint v) public {
        address kept = logic;
        (bool ok, ) = logic.delegatecall(abi.encodeWithSignature("set(uint256)", v));
    }
}
