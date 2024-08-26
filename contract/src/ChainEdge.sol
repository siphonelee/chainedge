// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import "@openzeppelin/contracts/access/Ownable.sol";

contract ChainEdge is Ownable {
    mapping(string => bool) cached;
    string[] public links;
    mapping(string => uint256) indexOf;

    mapping(address => uint256) servCount;

    event NewLink(string link);
    event RemoveLink(string link);
 
    constructor() Ownable(msg.sender) {
    }
   
    function removeFromCDN(string[] memory lnks) public onlyOwner {
        for (uint256 i = 0; i < lnks.length; ++i) {
            string memory key = lnks[i];

            if (!cached[key]) {
                continue;
            }

            delete cached[key];
            
            uint256 index = indexOf[key];
            delete indexOf[key];

            string memory lastLink = links[links.length - 1];
            indexOf[lastLink] = index;
            links[index] = lastLink;
            links.pop(); 
            
            // EVENT	
            emit RemoveLink(key);
        }
    }

    function addToCDN(string[] memory lnks) public onlyOwner {
        for (uint256 i = 0; i < lnks.length; ++i) {
            string memory key = lnks[i];
            if (!cached[key]) {
                cached[key] = true;
                indexOf[key] = links.length;
                links.push(key);
            }
          
            // EVENT
            emit NewLink(key);
        }
    }
     
    function addServeCount(uint256 cnt) public returns (uint256 v) {
        v = servCount[msg.sender];
        v += cnt;
        servCount[msg.sender] = v;
    } 
 
    function getCDNList() public view returns (string[] memory lnks) {
        lnks = links; 
    }
    
    function getIndexOf(string memory lnk) public view returns (uint256) {
        require(cached[lnk], "not existing");
        return indexOf[lnk];
    }

    function getServeCount() public view returns (uint256) {
        return servCount[msg.sender];
    }
}
