pragma solidity ^0.8.24;

interface IERC721 {
    function ownerOf(uint tokenId) external view returns (address);
    function transferFrom(address from, address to, uint tokenId) external;
}

contract ERC721 is IERC721 {
    mapping(uint => address) public owners;

    event Transfer(address indexed from, address indexed to, uint indexed tokenId);

    function ownerOf(uint tokenId) public view override returns (address) {
        address owner = owners[tokenId];
        return owner;
    }

    function transferFrom(address from, address to, uint tokenId) public override {
        address holder = owners[tokenId];
        holder = to;
    }
}
