pragma solidity ^0.8.24;

contract Function {
    function returnMany() public pure returns (uint, bool, uint) {
        return (1, true, 2);
    }

    function named() public pure returns (uint x, bool b, uint y) {
        return (1, true, 2);
    }

    function assigned() public pure returns (uint x, bool b, uint y) {
        x = 1;
        b = true;
        y = 2;
    }

    function arrayInput(uint[] memory _arr) public returns (uint[] memory) {
        uint[] memory a = _arr;
        return a;
    }

    uint[] public arr;

    function arrayOutput() public view returns (uint[] memory) {
        return arr;
    }
}
