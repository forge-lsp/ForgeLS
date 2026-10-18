pragma solidity ^0.8.24;

contract DataLocations {
    uint[] public arr;
    mapping(uint => address) map;
    struct MyStruct {
        uint foo;
    }
    mapping(uint => MyStruct) myStructs;

    function f(uint[] storage _arr, MyStruct storage _s) public {
        arr = _arr;
    }

    function g(uint[] memory _arr) public returns (uint[] memory) {
        uint[] memory a = _arr;
        return a;
    }

    function h(string calldata _text) public pure returns (string memory) {
        string memory m = _text;
        return m;
    }
}
