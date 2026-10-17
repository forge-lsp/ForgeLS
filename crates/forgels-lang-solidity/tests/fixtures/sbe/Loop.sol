pragma solidity ^0.8.24;

contract Loop {
    function loop() public pure {
        for (uint i = 0; i < 10; i++) {
            if (i == 3) {
                continue;
            }
        }
        uint j = 0;
        while (j < 10) {
            j++;
        }
    }
}
