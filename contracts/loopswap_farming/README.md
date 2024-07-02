# LoopSwap Farming

## Overview
  LoopSwap comes with the feature of Staking LP tokens and earning the reward in the 
form of various reward tokens. 
User needs some "LP Tokens" to enter into a Farm with. Farms can only accept their 
own exact LP Token. For example, the LOOP-UST Farm will only accept LOOP-UST 
LP Tokens. To get the exact LP Token, user will need to provide liquidity for that trading 
pair.  
Main functionalities are given below: 
-  Users can stake the LP tokens. 
-  Admin decides the Number of total reward tokens to be distributed to the pool 
as a reward. 
- The distribute function is called after which reward tokens will be distributed. 
- Proportionate to user’s staked LP tokens as compared to the total USD value of 
the reward pool, user share in pool reward will be calculated. 
-  User can then unstake and claim his reward. 

## Methodology:

  The contract is primarily based on creating a “LoopFarm” token for each pool. User 
will stakes his LP tokens in a particular pool, the contract will save it's token in the
pool. User will get reward on the basis of staked token and reward will be increased
due to subsequent distributions of the reward tokens by the admin. 
The contract keeps track of the LP staked by a user through the hashmap USER_STAKED_AMOUNT 
so that during unstaking and claiming, user can get his due LP tokens returned to him. 
Additionally, the functions of unstaking and claiming have been fused together so that 
those users who keep their LP tokens staked are benefitted. For instance, if a user 
decides to “unstake and claim”, his LP tokens will be unstaked and rewards will be 
claimed.


# Functions

```sh
pub fn instantiate():
```

This function will be used to Initialize the smart contract. It needs no parameter to 
initialize. 

```sh
pub fn execute_update_config():
```

This function is called by the admin when admin wants to change ownership of the 
contract.

```sh
pub fn execute_add_stakeable_token():
```
This function will allow admin to add the assets to be staked. In the start it performs the 
authorization check through state.owner and then checks if token already exists in 
the stakeable token list. Finally, it allows the admin to add new stakeable tokens 
corresponding to that particular pool based on what user share in reward will be calculated.

```sh
pub fn execute_add_distribution_token():
```
This function will allow admin to add the assets to be distributed as a reward e.g. LOOP 
tokens etc. to the users staking LP tokens. In the start it performs the authorization check 
through state.owner and then checks if token already exists in the distribution list. 
Finally, it allows the admin to add new distribution assets.

```sh
pub fn execute_stake():
```
This function will be called by the contract to stake user LP tokens when user sends 
asset to the contract in accordance with the receive interface of the contract. 
It will take only one parameter i.e. asset - the details of LP tokens user wants to stake. 
First it will check if the asset user wants to stake is among the stackable assets. It keeps 
a record of the staked value through USER_STAKED_AMOUNT and also maintains the record of 
TOTAL_STAKED in the pool. 

```sh
pub fn execute_update_reward():
```
This function will update daily reward for a pool. 
It will take two parameters  
- array of reward tokens 
- pool token address 
 
First of all, it will check whether the pool exists or not. Then it will check for each 
reward admin wants to update whether it exists in the distributable tokens or not. Then it
will update the reward.

```sh
pub fn execute_distribute():
```
This will be called by the admin to distribute daily rewards as per data after certian amount 
of time in pub fn execute_distribute()

```sh
pub fn execute_distribute_by_limit():
```
This will be called by the admin to distribute pagination implemented daily rewards as per data 
and will distribute the reward by the ratio of last distributed time to current time in pub fn 
execute_distribute_by_limit()


```sh
pub fn execute_unstake and claim():
```
This function will be called by the contract to unstake the LP tokens of the user and 
claim his rewards unstake_and_claim message in accordance with the receiver interface of the 
contract. 
It will take only one parameter i.e. stakeable token/pool address. The contracts will unstake
it's LP token which are in map USER_STAKED_AMOUNT and then transfers the rewards 
that the user had accumulated in the particular pool. 
The function updates USER_STAKED_AMOUNT, TOTAL STAKED, and TOTAL REWARD IN POOL. 

```sh
pub fn claim():
```
This function will be called by the contract to claim the calculated reward upto that point without
unstaking LP Tokens


# Queries

```sh
pub fn query_reward in pool():
```
This query will return the total available reward of a particular distribution token in the 
specific pool. It requires both pool and distribution token addresses in order to 
load value from a hashmap.

```sh
pub fn query_reward token to user():
```
This query will return the FLP tokens minted to the user for a particular staking pool 
through the hashmap REWARD_TOKEN_ISSUED.

```sh
pub fn query_list of distributable tokens by pool ():
```
This query will return all the distributable tokens (rewards) that are available for 
claiming in the given pool through the fn query pool rewards.

```sh
pub fn query_user_reward_in_pool ():
```
This query will return the user reward – all distribution tokens in a particular pool – 
based on user staked LP tokens.

```sh
pub fn query_staked_by_user():
```
This query will return the value of LP tokens user has staked through the hashmap 
LP_PROVIDED.

```sh
pub fn query_total staked():
```
This query will return the total staked amount in the pool by all the users through the 
hashmap TOTAL_STAKED.


```sh
pub fn query_list_of_stakeable_tokens(): 
```
This query will return the list of all stakeable tokens (pools).

```sh
pub fn query_list_of_distributable_tokens():
```
This query will the list of all distributable tokens (rewards).

```sh
pub fn query_stakeable info(): 
```
This is for testing only and returns the STAKEABLE_INFOS.

```sh
pub fn query_get_distibuteable_token_balance
```
this will return the unclaimed ditributed token amount in contract.

