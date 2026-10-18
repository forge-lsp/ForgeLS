pragma solidity ^0.8.24;

contract Deployer {
    event Deployed(address addr);

    function deploy(bytes memory code) public returns (address) {
        bytes memory prev = code;
        assembly {
            let a := create(0, add(code, 32), mload(code))
        }
        address next = msg.sender;
        return next;
    }
}
