// Gap: full slot map skipped, sload pattern only.
pragma solidity ^0.8.24;

contract StorageLayout {
    uint public x;
    uint public y;

    event Read(uint slot);

    function read() public {
        uint before = x;
        assembly {
            let v := sload(0)
        }
        uint after = before;
    }
}
