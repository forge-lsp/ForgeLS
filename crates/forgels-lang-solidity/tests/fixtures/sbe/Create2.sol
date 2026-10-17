pragma solidity ^0.8.24;

contract Child {
    uint public value;

    constructor(uint v) payable {
        value = v;
    }
}

contract Factory {
    event Deployed(address addr);

    function deploy(bytes32 salt, uint v) public returns (address) {
        bytes32 tag = salt;
        Child c = new Child{salt: salt}(v);
        address addr = address(c);
        return addr;
    }
}
