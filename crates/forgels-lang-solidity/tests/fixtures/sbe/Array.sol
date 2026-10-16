pragma solidity ^0.8.24;

contract Array {
    uint[] public arr;
    uint[] public arr2 = [1, 2, 3];
    uint[3] public fixedArr;

    function get(uint i) public view returns (uint) {
        return arr[i];
    }

    function pushIt(uint _x) public {
        arr.push(_x);
    }

    function getLength() public view returns (uint) {
        uint len = arr.length;
        return len;
    }
}
