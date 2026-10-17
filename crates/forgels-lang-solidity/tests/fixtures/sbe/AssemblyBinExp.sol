pragma solidity ^0.8.24;

contract BinExp {
    function pow(uint base, uint exp) public pure returns (uint) {
        uint before = base;
        assembly {
            let r := exp(base, exp)
        }
        uint after = before;
        return after;
    }
}
