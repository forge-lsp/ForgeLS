pragma solidity ^0.8.24;

contract VaultPriv {
    bytes32 private seed;
    uint private locked;

    event Revealed(bytes32 slot);

    function peek() public {
        bytes32 got = seed;
        assembly {
            let v := sload(0)
        }
        bytes32 next = got;
    }
}
