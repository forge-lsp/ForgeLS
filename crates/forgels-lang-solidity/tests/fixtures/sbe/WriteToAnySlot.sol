pragma solidity ^0.8.24;

contract SlotWriter {
    function write(uint slot, uint val) public {
        uint before = slot;
        assembly {
            sstore(slot, val)
        }
        uint after = before;
    }
}
