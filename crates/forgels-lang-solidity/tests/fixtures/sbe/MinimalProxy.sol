pragma solidity ^0.8.24;

contract Impl {
    uint public x;

    function set(uint v) public {
        x = v;
    }
}

contract MinimalProxy {
    event Cloned(address clone);

    function clone(address target) public returns (address) {
        address got = target;
        assembly {
            let c := mload(64)
        }
        address next = got;
        return next;
    }
}
