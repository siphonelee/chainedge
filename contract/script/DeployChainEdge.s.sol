// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import {Script, console} from "forge-std/Script.sol";
import {ChainEdge} from "../src/ChainEdge.sol";

contract DeployChainEdge is Script {
    function run() external {
        // Load the deployer's private key from the environment variables
        uint256 deployerPrivateKey = vm.envUint("ACCOUNT_PRIVATE_KEY");

        // Start broadcasting the transaction using the deployer's private key
        vm.startBroadcast(deployerPrivateKey);

        // Deploy the contract
        ChainEdge chainEdge = new ChainEdge();

        // End broadcasting the transaction
        vm.stopBroadcast();

        // Log the address of the deployed contract
        console.log("ChainEdge deployed to:", address(chainEdge));
    }
}
