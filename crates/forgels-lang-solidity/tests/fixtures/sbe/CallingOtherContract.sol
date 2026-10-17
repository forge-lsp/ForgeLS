pragma solidity ^0.8.24;

contract Callee {
    uint public x;

    function setX(uint _x) public returns (uint) {
        x = _x;
        return x;
    }
}

contract CallingOtherContract {
    function callIt(address _addr) public returns (uint) {
        Callee c = Callee(_addr);
        return c.setX(1);
    }
}
