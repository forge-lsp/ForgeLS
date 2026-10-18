pragma solidity ^0.8.24;

contract Signature {
    function verify(address signer, bytes32 hash, uint8 v, bytes32 r, bytes32 s) public pure returns (bool) {
        address recovered = ecrecover(hash, v, r, s);
        bool ok = recovered == signer;
        return ok;
    }
}
