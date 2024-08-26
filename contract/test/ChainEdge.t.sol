// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import {Test, console} from "forge-std/Test.sol";
import {ChainEdge} from "../src/ChainEdge.sol";

contract ChainEdgeTest is Test {
    ChainEdge public chainEdge;

    function setUp() public {
        chainEdge = new ChainEdge();
    }

    function testAddRemove() public {
        string[] memory arr = new string[](4);
        arr[0] = "get@/url1/x?a=b";
        arr[1] = "post@/url2";
        arr[2] = "get@/url3";
        arr[3] = "get@/url=4";
        chainEdge.addToCDN(arr);
        string[] memory lst = chainEdge.getCDNList();
        assertEq(lst.length, 4);

        string[] memory del_arr = new string[](2);
        del_arr[0] = "post@/url2";
        del_arr[1] = "not_existing";
        chainEdge.removeFromCDN(del_arr);       
        lst = chainEdge.getCDNList();
        assertEq(lst.length, 3);
        assertEq(chainEdge.getIndexOf("get@/url1/x?a=b"), 0);
        assertEq(chainEdge.getIndexOf("get@/url=4"), 1);
        assertEq(chainEdge.getIndexOf("get@/url3"), 2);
        
        del_arr[0] = "get@/url3";
        chainEdge.removeFromCDN(del_arr);       
        lst = chainEdge.getCDNList();
        assertEq(lst.length, 2);
        assertEq(chainEdge.getIndexOf("get@/url1/x?a=b"), 0);
        assertEq(chainEdge.getIndexOf("get@/url=4"), 1);
    }
    
    function testAddCount() public {
        uint256 res = chainEdge.addServeCount(5);
        assertEq(res, 5);
        
        assertEq(chainEdge.getServeCount(), 5);
    }
}
