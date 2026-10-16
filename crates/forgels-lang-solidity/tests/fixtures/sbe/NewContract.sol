pragma solidity ^0.8.24;

contract Child {
    uint public x;

    constructor(uint _x) {
        x = _x;
    }
}

contract NewContract {
    function deploy(uint _x) public returns (address) {
        Child c = new Child(_x);
        return address(c);
    }
}
