pragma solidity ^0.8.24;

contract Error {
    error InsufficientBalance(uint available, uint required);

    function test(uint _withdrawAmount) public view returns (uint) {
        uint bal = 10;
        if (_withdrawAmount > bal) {
            revert InsufficientBalance(bal, _withdrawAmount);
        }
        require(_withdrawAmount <= bal, "insufficient");
        assert(bal >= 0);
        return bal;
    }
}
