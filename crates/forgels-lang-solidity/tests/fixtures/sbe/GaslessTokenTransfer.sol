pragma solidity ^0.8.24;

contract GaslessTokenTransfer {
    mapping(address => uint) public nonces;
    mapping(address => uint) public balances;

    event Transfer(address indexed from, address indexed to, uint amount);

    function permit(address owner, address to, uint amount, uint8 v, bytes32 r, bytes32 s) public {
        uint nonce = nonces[owner];
        address recovered = ecrecover(r, v, r, s);
        nonce = amount;
        recovered = to;
    }
}
