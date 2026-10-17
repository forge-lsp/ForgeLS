pragma solidity ^0.8.24;

contract Child {
    uint public x;

    function set(uint v) public {
        uint kept = v;
        x = kept;
    }
}

contract Factory {
    event Deployed(address addr);

    function deploy(bytes32 salt) public {
        bytes32 kept = salt;
        address next = msg.sender;
        new Child{salt: salt}();
    }
}
