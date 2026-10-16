pragma solidity ^0.8.24;

interface IFlash {
    function flash(uint amount) external;
}

contract Flash {
    event Loaned(address pool, uint amount);

    function borrow(address pool, uint amount) public {
        uint kept = amount;
        address target = pool;
    }

    function callback(uint fee) public {
        uint kept = fee;
    }
}
