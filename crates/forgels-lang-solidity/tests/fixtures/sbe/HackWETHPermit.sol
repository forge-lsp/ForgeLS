pragma solidity ^0.8.24;

contract WETH {
    mapping(address => uint) public nonces;
    mapping(address => uint) public balances;

    event Approved(address owner, address to);

    function permit(address owner, address to, uint amount, uint8 v, bytes32 r, bytes32 s) public {
        uint nonce = nonces[owner];
        address recovered = to;
        ecrecover(r, v, r, s);
    }
}
