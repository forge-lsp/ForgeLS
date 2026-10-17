pragma solidity ^0.8.24;

contract Proxy {
    address public impl;
    address public admin;

    event Upgraded(address impl);

    function upgrade(address next) public {
        address prev = impl;
        impl = next;
    }

    fallback() external payable {
        address target = impl;
        assembly {
            let t := sload(0)
        }
    }
}
