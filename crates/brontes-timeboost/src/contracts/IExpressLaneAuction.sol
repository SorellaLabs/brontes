// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.0;

import {IERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import {RoundTimingInfo} from "./RoundTimingInfo.sol";
import {ELCRound} from "./ELCRound.sol";
import {IAccessControlEnumerableUpgradeable} from
    "@openzeppelin/contracts-upgradeable/access/IAccessControlEnumerableUpgradeable.sol";
import {IERC165Upgradeable} from
    "@openzeppelin/contracts-upgradeable/utils/introspection/IERC165Upgradeable.sol";

/// @notice A bid to control the express lane for a specific round
struct Bid {
    /// @notice The address to be set as the express lane controller if this bid wins the auction round
    address expressLaneController;
    /// @notice The maximum amount the bidder is willing to pay if they win the round
    ///         The auction is a second price auction, so the winner may end up paying less than this amount
    ///         however this is the maximum amount up to which they may have to pay
    uint256 amount;
    /// @notice Authentication of this bid by the bidder.
    ///         The bidder signs the 712 hash of the struct Bid(uint64 round,address expressLaneController,uint256 amount)
    bytes signature;
}

/// @notice Sets a transferor for an express lane controller
///         The transferor is an address that will have the right to transfer express lane controller rights
///         on behalf an express lane controller.
struct Transferor {
    /// @notice The address of the transferor
    address addr;
    /// @notice The express lane controller can choose to fix the transferor until a future round number
    ///         This gives them ability to guarantee to other parties that they will not change transferor during an ongoing round
    ///         The express lane controller can ignore this feature by setting this value to 0.
    uint64 fixedUntilRound;
}

/// @notice The arguments used to initialize an express lane auction
struct InitArgs {
    /// @notice The address who can resolve auctions
    address _auctioneer;
    /// @notice The ERC20 token that bids will be made in
    ///         It is assumed that this token does NOT have fee-on-transfer, rebasing,
    ///         transfer hooks or otherwise non-standard ERC20 logic.
    address _biddingToken;
    /// @notice The address to which auction winners will pay the bid
    address _beneficiary;
    /// @notice Round timing components: offset, auction closing, round duration, reserve submission
    RoundTimingInfo _roundTimingInfo;
    /// @notice The minimum reserve price, also used to set the initial reserve price
    uint256 _minReservePrice;
    /// @notice Can update the auctioneer address
    address _auctioneerAdmin;
    /// @notice The address given the rights to change the min reserve price
    address _minReservePriceSetter;
    /// @notice The address given the rights to change the reserve price
    address _reservePriceSetter;
    /// @notice Can update the reserve price setter address
    address _reservePriceSetterAdmin;
    /// @notice The address given the rights to change the beneficiary address
    address _beneficiarySetter;
    /// @notice _roundTimingSetter The address given the rights to update the round timing info
    address _roundTimingSetter;
    /// @notice The admin that can manage all the admin roles in the contract
    address _masterAdmin;
}

interface IExpressLaneAuction is IAccessControlEnumerableUpgradeable, IERC165Upgradeable {
    /// @notice An account has deposited funds to be used for bidding in the auction
    /// @param account The account that deposited funds
    /// @param amount The amount deposited by that account
    event Deposit(address indexed account, uint256 amount);

    /// @notice An account has initiated a withdrawal of funds from the auction
    /// @param account The account withdrawing the funds
    /// @param withdrawalAmount The amount beind withdrawn
    /// @param roundWithdrawable The round the funds will become withdrawable in
    event WithdrawalInitiated(
        address indexed account, uint256 withdrawalAmount, uint256 roundWithdrawable
    );

    /// @notice An account has finalized a withdrawal
    /// @param account The account that finalized the withdrawal
    /// @param withdrawalAmount The amount that was withdrawn
    event WithdrawalFinalized(address indexed account, uint256 withdrawalAmount);

    /// @notice An auction was resolved and a new express lane controller was set
    /// @param isMultiBidAuction Whether there was more than one bid in the auction
    /// @param round The round for which the rights were being auctioned
    /// @param firstPriceBidder The bidder who won the auction
    /// @param firstPriceExpressLaneController The address that will have express lane control in the specified round
    /// @param firstPriceAmount The price in the winning bid
    /// @param price The price paid by the winning bidder
    /// @param roundStartTimestamp The time at which the round will start
    /// @param roundEndTimestamp The time at which the round will end
    event AuctionResolved(
        bool indexed isMultiBidAuction,
        uint64 round,
        address indexed firstPriceBidder,
        address indexed firstPriceExpressLaneController,
        uint256 firstPriceAmount,
        uint256 price,
        uint64 roundStartTimestamp,
        uint64 roundEndTimestamp
    );

    /// @notice A new express lane controller was set
    /// @param round The round which the express lane controller will control
    /// @param previousExpressLaneController The previous express lane controller
    /// @param newExpressLaneController The new express lane controller
    /// @param transferor The address that transferored the controller rights. The transferor if set, otherwise the express lane controller
    /// @param startTimestamp The timestamp at which the new express lane controller takes over
    /// @param endTimestamp The timestamp at which the new express lane controller's rights are expected to cease. They can cease earlier if
    ///                     if they are transfered before the end of the round
    event SetExpressLaneController(
        uint64 round,
        address indexed previousExpressLaneController,
        address indexed newExpressLaneController,
        address indexed transferor,
        uint64 startTimestamp,
        uint64 endTimestamp
    );

    /// @notice A new transferor has been set for
    /// @param expressLaneController The express lane controller that has a transferor
    /// @param transferor The transferor chosen
    /// @param fixedUntilRound The round until which this transferor is fixed for this controller
    event SetTransferor(
        address indexed expressLaneController, address indexed transferor, uint64 fixedUntilRound
    );

    /// @notice The minimum reserve price was set
    /// @param oldPrice The previous minimum reserve price
    /// @param newPrice The new minimum reserve price
    event SetMinReservePrice(uint256 oldPrice, uint256 newPrice);

    /// @notice A new reserve price was set
    /// @param oldReservePrice Previous reserve price
    /// @param newReservePrice New reserve price
    event SetReservePrice(uint256 oldReservePrice, uint256 newReservePrice);

    /// @notice A new beneficiary was set
    /// @param oldBeneficiary The previous beneficiary
    /// @param newBeneficiary The new beneficiary
    event SetBeneficiary(address oldBeneficiary, address newBeneficiary);

    /// @notice A new round timing info has been set
    /// @param currentRound The round during which the timing info was set
    /// @param offsetTimestamp The new offset timestamp
    /// @param roundDurationSeconds The new round duration seconds
    /// @param auctionClosingSeconds The new auction closing seconds
    /// @param reserveSubmissionSeconds The new reserve submission seconds
    event SetRoundTimingInfo(
        uint64 currentRound,
        int64 offsetTimestamp,
        uint64 roundDurationSeconds,
        uint64 auctionClosingSeconds,
        uint64 reserveSubmissionSeconds
    );

    /// @notice The role given to the address that can resolve auctions
    function AUCTIONEER_ROLE() external returns (bytes32);

    /// @notice The role that administers AUCTIONEER_ROLE
    function AUCTIONEER_ADMIN_ROLE() external returns (bytes32);

    /// @notice The role given to the address that can set the minimum reserve
    function MIN_RESERVE_SETTER_ROLE() external returns (bytes32);

    /// @notice The role given to the address that can set the reserve
    function RESERVE_SETTER_ROLE() external returns (bytes32);

    /// @notice The role that administers the RESERVE_SETTER_ROLE
    function RESERVE_SETTER_ADMIN_ROLE() external returns (bytes32);

    /// @notice The role given to the address that can set the beneficiary
    function BENEFICIARY_SETTER_ROLE() external returns (bytes32);

    /// @notice The role given to addresses that can set round timing info
    function ROUND_TIMING_SETTER_ROLE() external returns (bytes32);

    /// @notice The beneficiary who receives the funds that are paid by the auction winners
    function beneficiary() external returns (address);

    /// @notice The ERC20 token that can be used for bidding
    ///         It is assumed that the this token does NOT have fee-on-transfer, rebasing,
    ///         transfer hooks or otherwise non-standard ERC20 logic.
    function biddingToken() external returns (IERC20);

    /// @notice The reserve price for the auctions. The reserve price setter can update this value
    ///         to ensure that controlling rights are auctioned off at a reasonable value
    function reservePrice() external returns (uint256);

    /// @notice The minimum amount the reserve can be set to. This ensures that reserve prices cannot be
    ///         set too low
    function minReservePrice() external returns (uint256);

    /// @notice Returns the currently unflushed balance of the beneficiary
    ///         Anyone can call flushBalance to transfer this balance from the auction to the beneficiary
    ///         This is a gas optimisation to avoid making a transfer every time an auction is resolved
    function beneficiaryBalance() external returns (uint256);

    /// @notice Express lane controllers can optionally set a transferor address that has the rights
    ///         to transfer their controller rights. This function returns the transferor if one has been set
    ///         Returns the transferor for the supplied controller, and the round until which this
    ///         transferor is fixed if set.
    function transferorOf(
        address expressLaneController
    ) external returns (address addr, uint64 fixedUntil);

    /// @notice Initialize the auction
    /// @param args Initialization parameters
    function initialize(
        InitArgs memory args
    ) external;

    /// @notice Round timing components: offset, auction closing, round duration and reserve submission
    function roundTimingInfo()
        external
        view
        returns (
            int64 offsetTimestamp,
            uint64 roundDurationSeconds,
            uint64 auctionClosingSeconds,
            uint64 reserveSubmissionSeconds
        );

    /// @notice The current auction round that we're in
    ///         Bidding for control of the next round occurs in the current round
    function currentRound() external view returns (uint64);

    /// @notice Is the current auction round closed for bidding
    ///         After the round has closed the auctioneer can resolve it with the highest bids
    ///         Note. This can change unexpectedly if a round timing info is updated
    function isAuctionRoundClosed() external view returns (bool);

    /// @notice The auction reserve cannot be updated during the blackout period
    ///         This starts ReserveSubmissionSeconds before the round closes and ends when the round is resolved, or the round ends
    ///         Note. This can change unexpectedly if a round timing info is updated
    function isReserveBlackout() external view returns (bool);

    /// @notice Gets the start and end timestamps for a given round
    ///         This only returns the start and end timestamps given the current round timing info, which can be updated
    ///         Historical round timestamp can be found by checking the logs for round timing info updates, or by looking
    ///         at the timing info emitted in events from resolved auctions
    ///         Since it is possible to set a negative offset, the start and end time may also be negative
    ///         In this case requesting roundTimestamps will revert.
    /// @param round The round to find the timestamps for
    /// @return start The start of the round in seconds, inclusive
    /// @return end The end of the round in seconds, inclusive
    function roundTimestamps(
        uint64 round
    ) external view returns (uint64 start, uint64 end);

    /// @notice Update the beneficiary to a new address
    ///         Setting the beneficiary does not flush any pending balance, so anyone calling this function should consider
    ///         whether they want to flush before calling set.
    ///         It is expected that the DAO will have the rights to set beneficiary, and since they execute calls via
    ///         action contract they can atomically call flush and set beneficiary together.
    /// @param newBeneficiary The new beneficiary
    function setBeneficiary(
        address newBeneficiary
    ) external;

    /// @notice Set the minimum reserve. The reserve cannot be set below this value
    ///         Having a minimum reserve ensures that the reserve setter doesn't set the reserve too low
    ///         If the new minimum reserve is greater than the current reserve then the reserve will also be set,
    ///         this will happen regardless of whether we are in a reserve blackout period or not.
    ///         The min reserve setter is therefore trusted to either give bidders plenty of notice that they may change the min
    ///         reserve, or do so outside of the blackout window. It is expected that the min reserve setter will be controlled by
    ///         Arbitrum DAO who can only make changes via timelocks, thereby providing the notice to bidders.
    ///         If the new minimum reserve is set to a very high value eg max(uint) then the auction will never be able to resolve
    ///         the min reserve setter is therefore trusted not to do this as it would DOS the auction. Note that even if this occurs
    ///         bidders will not lose their funds and will still be able to withdraw them.
    /// @param newMinReservePrice The new minimum reserve
    function setMinReservePrice(
        uint256 newMinReservePrice
    ) external;

    /// @notice Set the auction reserve price. Must be greater than or equal the minimum reserve.
    ///         A reserve price setter is given the ability to change the reserve price to ensure that express lane control rights
    ///         are not sold off too cheaply. They are trusted to set realistic values for this.
    ///         However they can only change this value when not in the blackout period, which occurs before at the auction close
    ///         This ensures that bidders will have plenty of time to observe the reserve before the auction closes, and that
    ///         the reserve cannot be changed at the last second. One exception to this is if the minimum reserve changes, see the setMinReservePrice
    ///         documentation for more details.
    ///         If the new reserve is set to a very high value eg max(uint) then the auction will never be able to resolve
    ///         the reserve setter is therefore trusted not to do this as it would DOS the auction. Note that even if this occurs
    ///         bidders will not lose their funds and will still be able to withdraw them.
    ///         Note to reserve price setter, setting reserve price is dependent on the time into the round, which can change if the round timing info is updated
    /// @param newReservePrice The price to set the reserve to
    function setReservePrice(
        uint256 newReservePrice
    ) external;

    /// @notice Sets new round timing info. When setting a new round timing info the current round and the start
    ///         timestamp of the next round cannot change. The caller can ensure this by dynamically calculating
    ///         the correct offset which will produce this for the specified round duration seconds in the new timing info.
    ///         Changing timing info affects the current ongoing auction, given that the round may already have been resolved
    ///         this could result in bidders paying for a round that is longer or shorter than they expected. To that end
    ///         the round timing setter is trusted not to set this function too often, and any observers who depend upon this timing info
    ///         (eg bidders, auctioneer, reserve price setter etc) should be able to see when this is going to happen.
    ///         On Arbitrum One the expected round timing setter is the Arbitrum DAO, that can only
    ///         make changes by passing proposals through timelocks, therefore providing the notice to bidders.
    ///         Since the next round of the new info must be the same as the next round of the current info, it follows
    ///         that the update can only be made within min(roundDuration, newRoundDuration) of the end of the round, making
    ///         an update outside of this will cause a revert.
    ///         If necessary negative offsets can be set in order to achieve the next round number, given that
    ///         the maximum round duration is 1 day it should be possible to have many thousands of years worth of
    ///         rounds before it is not longer possible (due to int underflow) to change from 1 second to 1 day duration
    /// @param newRoundTimingInfo The new timing info to set
    function setRoundTimingInfo(
        RoundTimingInfo calldata newRoundTimingInfo
    ) external;

    /// @notice Get the current balance of specified account.
    ///         If a withdrawal is initiated this balance will reduce in current round + 2
    /// @param account The specified account
    function balanceOf(
        address account
    ) external view returns (uint256);

    /// @notice Get what the balance will be at some future round
    ///         Since withdrawals are scheduled for future rounds it is possible to see that a balance
    ///         will reduce at some future round, this method provides a way to query that.
    ///         Specifically this will return 0 if the withdrawal round has been set, and is < the supplied round
    ///         Will revert if a round from the past is supplied
    /// @param account The specified account
    /// @param round The round to query the balance at
    function balanceOfAtRound(address account, uint64 round) external view returns (uint256);

    /// @notice The amount of balance that can currently be withdrawn via the finalize method
    ///         This balance only increases current round + 2 after a withdrawal is initiated
    /// @param account The account the check the withdrawable balance for
    function withdrawableBalance(
        address account
    ) external view returns (uint256);

    /// @notice The amount of balance that can currently be withdrawn via the finalize method
    ///         Since withdrawals are scheduled for future rounds it is possible to see that a withdrawal balance
    ///         will increase at some future round, this method provides a way to query that.
    ///         Specifically this will return 0 unless the withdrawal round has been set, and is >= the supplied round
    ///         Will revert if a round from the past is supplied
    ///         This balance only increases current round + 2 after a withdrawal is initiated
    /// @param account The account the check the withdrawable balance for
    /// @param round The round to query the withdrawable balance at
    function withdrawableBalanceAtRound(
        address account,
        uint64 round
    ) external view returns (uint256);

    /// @notice Deposit an amount of ERC20 token to the auction to make bids with
    ///         Deposits must be submitted prior to bidding.
    ///         When withdrawing the full balance must be withdrawn. This is done via a two step process
    ///         of initialization and finalization, which takes at least 2 rounds to complete.
    ///         The round timing info offset timestamp is the start of the zeroth round, so if deposits
    ///         are made before that time they will need to wait until 2 rounds after that offset has occurred
    /// @dev    Deposits are submitted first so that the auctioneer can be sure that the accepted bids can actually be paid
    /// @param amount   The amount to deposit.
    function deposit(
        uint256 amount
    ) external;

    /// @notice Initiate a withdrawal of the full account balance of the message sender
    ///         Once funds have been deposited they can only be retrieved by initiating + finalizing a withdrawal
    ///         There is a delay between initializing and finalizing a withdrawal so that the auctioneer can be sure
    ///         that value cannot be removed before an auction is resolved. The timeline is as follows:
    ///         1. Initiate a withdrawal at some time in round r
    ///         2. During round r the balance is still available and can be used in an auction
    ///         3. During round r+1 the auctioneer should consider any accounts that have been initiated for withdrawal as having zero balance
    ///            However if a bid is submitted the balance will be available for use
    ///         4. During round r+2 the bidder can finalize a withdrawal and remove their funds
    ///         A bidder may have only one withdrawal being processed at any one time, and that withdrawal will be for the full balance
    function initiateWithdrawal() external;

    /// @notice Finalizes a withdrawal and transfers the funds to the msg.sender
    ///         Withdrawals can only be finalized 2 rounds after being initiated
    function finalizeWithdrawal() external;

    /// @notice Can be called by anyone to transfer any beneficiary balance from the auction contract to the beneficiary
    ///         This is not done separately so that it does not need to be done during auction resolution, thus saving some gas costs there
    function flushBeneficiaryBalance() external;

    /// @notice The domain separator used in the 712 signing hash
    function domainSeparator() external view returns (bytes32);

    /// @notice Get the 712 hash of a bid used for signing
    /// @param round The round the bid is for the control of
    /// @param expressLaneController The address that will be the express lane controller if the bid wins
    /// @param amount The amount being bid
    function getBidHash(
        uint64 round,
        address expressLaneController,
        uint256 amount
    ) external view returns (bytes32);

    /// @notice Resolve the auction with just a single bid. The auctioneer is trusted to call this only when there are
    ///         less than two bids higher than the reserve price for an auction round.
    ///         In this case the highest bidder will pay the reserve price for the round
    /// @dev    We do not enforce it, but the following accounts or their sybils, are trusted not to send bids to the auctioneer
    ///         Auctioneer, beneficiary, beneficiary setter, reserve price setter, min reserve price setter, role admin, round timing info setter
    /// @param firstPriceBid The highest price bid. Must have a price higher than the reserve. Price paid is the reserve
    function resolveSingleBidAuction(
        Bid calldata firstPriceBid
    ) external;

    /// @notice Resolves the auction round with the two highest bids for that round
    ///         The highest price bidder pays the price of the second highest bid
    ///         Both bids must be higher than the reserve
    /// @dev    We do not enforce it, but the following accounts or their sybils, are trusted not to send bids to the auctioneer
    ///         Auctioneer, beneficiary, beneficiary setter, reserve price setter, min reserve price setter, role admin, round timing info setter
    /// @param firstPriceBid The highest price bid
    /// @param secondPriceBid The second highest price bid
    function resolveMultiBidAuction(
        Bid calldata firstPriceBid,
        Bid calldata secondPriceBid
    ) external;

    /// @notice Sets a transferor for an express lane controller
    ///         The transferor is an address that will have the right to transfer express lane controller rights
    ///         on behalf an express lane controller.
    /// @param transferor The transferor to set
    function setTransferor(
        Transferor calldata transferor
    ) external;

    /// @notice Express lane controllers are allowed to transfer their express lane rights for the current or future
    ///         round to another address. They may use this for reselling their rights after purchasing them
    ///         Again, the priviledged accounts mentioned in resolve documentation are trusted not to try to receive rights via this message.
    ///         Although they cannot stop someone transferring the rights to them, they should not use the controller rights if that does occur
    /// @param round The round to transfer rights for
    /// @param newExpressLaneController The new express lane controller to transfer the rights to
    function transferExpressLaneController(
        uint64 round,
        address newExpressLaneController
    ) external;

    /// @notice The last two auction rounds that were resolved
    /// @return The most recent resolved auction round
    /// @return The second most recent resolved auction round
    function resolvedRounds() external view returns (ELCRound memory, ELCRound memory);
}