use crate::msg::{AskResponse, BidResponse, ExecuteMsg, QueryMsg};
use crate::state::SaleType;
use crate::testing::helpers::funds::{calculated_creator_balance_after_fairburn, listing_funds};
use crate::testing::helpers::nft_functions::{approve, mint, transfer};
use crate::testing::setup::setup_accounts::{setup_second_bidder_account, INITIAL_BALANCE};
use crate::testing::setup::setup_marketplace::{setup_marketplace, LISTING_FEE, MIN_EXPIRY};
use crate::testing::setup::templates::standard_minter_template;
use cosmwasm_std::{coin, coins, Timestamp, Uint128};
use cw_multi_test::Executor;
use sg_std::{GENESIS_MINT_START_TIME, NATIVE_DENOM};
use test_suite::common_setup::setup_accounts_and_block::setup_block_time;

#[test]
fn set_auction_bids() {
    let vt = standard_minter_template(1);
    let (mut router, creator, bidder) = (vt.router, vt.accts.creator, vt.accts.bidder);
    let marketplace = setup_marketplace(&mut router, creator.clone()).unwrap();
    let minter = vt.collection_response_vec[0].minter.clone().unwrap();
    let collection = vt.collection_response_vec[0].collection.clone().unwrap();
    let token_id = 1;
    let start_time = Timestamp::from_nanos(GENESIS_MINT_START_TIME);
    setup_block_time(&mut router, GENESIS_MINT_START_TIME, None);

    mint(&mut router, &creator, &minter);
    approve(&mut router, &creator, &collection, &marketplace, token_id);

    // An asking price is made by the creator
    let set_ask = ExecuteMsg::SetAsk {
        sale_type: SaleType::Auction,
        collection: collection.to_string(),
        token_id,
        price: coin(150, NATIVE_DENOM),
        funds_recipient: None,
        reserve_for: None,
        expires: start_time.plus_seconds(MIN_EXPIRY + 1),
        finders_fee_bps: Some(0),
    };

    let res = router.execute_contract(
        creator.clone(),
        marketplace.clone(),
        &set_ask,
        &listing_funds(LISTING_FEE).unwrap(),
    );
    assert!(res.is_ok());

    // Bidder makes bid
    let set_bid_msg = ExecuteMsg::SetBid {
        sale_type: SaleType::Auction,
        collection: collection.to_string(),
        token_id,
        finders_fee_bps: None,
        expires: start_time.plus_seconds(MIN_EXPIRY + 1),
        finder: None,
    };

    // Bidder makes bid lower than the asking price
    let res = router.execute_contract(
        bidder.clone(),
        marketplace.clone(),
        &set_bid_msg,
        &coins(100, NATIVE_DENOM),
    );
    assert!(res.is_err());

    // Bidder makes bid higher than the asking price
    let res = router.execute_contract(
        bidder.clone(),
        marketplace.clone(),
        &set_bid_msg,
        &coins(200, NATIVE_DENOM),
    );
    assert!(res.is_ok());

    let ask_query = QueryMsg::Ask {
        collection: collection.to_string(),
        token_id,
    };

    // ask should be returned
    let res: AskResponse = router
        .wrap()
        .query_wasm_smart(marketplace.clone(), &ask_query)
        .unwrap();
    assert_ne!(res.ask, None);

    let bid_query = QueryMsg::Bid {
        collection: collection.to_string(),
        token_id,
        bidder: bidder.to_string(),
    };

    // bid should be returned
    let res: BidResponse = router
        .wrap()
        .query_wasm_smart(marketplace.clone(), &bid_query)
        .unwrap();
    assert_ne!(res.bid, None);
    let bid = res.bid.unwrap();
    assert_eq!(bid.price, Uint128::from(200u128));

    let bidder2 = setup_second_bidder_account(&mut router).unwrap();

    // Bidder 2 makes bid equal to the asking price
    let set_bid_msg = ExecuteMsg::SetBid {
        sale_type: SaleType::FixedPrice,
        collection: collection.to_string(),
        token_id,
        finders_fee_bps: None,
        expires: start_time.plus_seconds(MIN_EXPIRY + 1),
        finder: None,
    };

    let res = router.execute_contract(
        bidder2.clone(),
        marketplace.clone(),
        &set_bid_msg,
        &coins(150, NATIVE_DENOM),
    );
    assert!(res.is_ok());

    // bid should be returned for bidder 1
    let res: BidResponse = router
        .wrap()
        .query_wasm_smart(marketplace.clone(), &bid_query)
        .unwrap();
    assert_ne!(res.bid, None);
    let bid = res.bid.unwrap();
    assert_eq!(bid.price, Uint128::from(200u128));

    let bid_query = QueryMsg::Bid {
        collection: collection.to_string(),
        token_id,
        bidder: bidder2.to_string(),
    };

    // bid should  be returned for bidder 2
    let res: BidResponse = router
        .wrap()
        .query_wasm_smart(marketplace.clone(), &bid_query)
        .unwrap();
    assert_ne!(res.bid, None);
    let bid = res.bid.unwrap();
    assert_eq!(bid.price, Uint128::from(150u128));

    // Creator accepts bid
    let accept_bid_msg = ExecuteMsg::AcceptBid {
        collection: collection.to_string(),
        token_id,
        bidder: bidder.to_string(),
        finder: None,
    };
    let res = router.execute_contract(creator.clone(), marketplace.clone(), &accept_bid_msg, &[]);
    assert!(res.is_ok());
    // ask should have been removed
    let res: AskResponse = router
        .wrap()
        .query_wasm_smart(marketplace.clone(), &ask_query)
        .unwrap();
    assert_eq!(res.ask, None);

    // Check creator has been paid
    let creator_native_balances = router.wrap().query_all_balances(creator.clone()).unwrap();
    let creator_balance_after_fee = calculated_creator_balance_after_fairburn();
    assert_eq!(
        creator_native_balances,
        coins(creator_balance_after_fee.u128() + 200 - 4, NATIVE_DENOM)
    );

    // Check contract has second bid balance
    let contract_balances = router.wrap().query_all_balances(marketplace).unwrap();
    assert_eq!(contract_balances, coins(150, NATIVE_DENOM));
}

#[test]
fn transfer_accept_bid() {
    let vt = standard_minter_template(1);
    let (mut router, creator, owner, bidder) =
        (vt.router, vt.accts.creator, vt.accts.owner, vt.accts.bidder);
    let marketplace = setup_marketplace(&mut router, creator.clone()).unwrap();
    let minter = vt.collection_response_vec[0].minter.clone().unwrap();
    let collection = vt.collection_response_vec[0].collection.clone().unwrap();
    let token_id = 1;
    let start_time = Timestamp::from_nanos(GENESIS_MINT_START_TIME);
    setup_block_time(&mut router, GENESIS_MINT_START_TIME, None);
    let owner_starting_balances = router.wrap().query_all_balances(owner.clone()).unwrap();

    mint(&mut router, &creator, &minter);
    approve(&mut router, &creator, &collection, &marketplace, token_id);

    // An asking price is made by the creator
    let set_ask = ExecuteMsg::SetAsk {
        sale_type: SaleType::Auction,
        collection: collection.to_string(),
        token_id,
        price: coin(150, NATIVE_DENOM),
        funds_recipient: None,
        reserve_for: None,
        expires: start_time.plus_seconds(MIN_EXPIRY + 1),
        finders_fee_bps: Some(0),
    };

    let res = router.execute_contract(
        creator.clone(),
        marketplace.clone(),
        &set_ask,
        &listing_funds(LISTING_FEE).unwrap(),
    );
    assert!(res.is_ok());

    // Bidder makes bid higher than the asking price
    let set_bid_msg = ExecuteMsg::SetBid {
        sale_type: SaleType::Auction,
        collection: collection.to_string(),
        token_id,
        finders_fee_bps: None,
        expires: start_time.plus_seconds(MIN_EXPIRY + 1),
        finder: None,
    };
    let res = router.execute_contract(
        bidder.clone(),
        marketplace.clone(),
        &set_bid_msg,
        &coins(200, NATIVE_DENOM),
    );
    assert!(res.is_ok());

    // Creator transfers token to owner
    transfer(&mut router, &creator, &owner, &collection, token_id);

    // Current owner accepts bid
    approve(&mut router, &owner, &collection, &marketplace, token_id);
    let accept_bid_msg = ExecuteMsg::AcceptBid {
        collection: collection.to_string(),
        token_id,
        bidder: bidder.to_string(),
        finder: None,
    };
    let res = router.execute_contract(owner.clone(), marketplace.clone(), &accept_bid_msg, &[]);
    assert!(res.is_ok());

    // Double check accounting
    // creator gets 200 * 10% more royalties
    let creator_native_balances = router.wrap().query_all_balances(creator.clone()).unwrap();
    assert_eq!(
        creator_native_balances,
        vec![coin(
            calculated_creator_balance_after_fairburn().u128() + 20,
            NATIVE_DENOM.to_string()
        )]
    );
    // bidder pays 200
    let bidder_native_balances = router.wrap().query_all_balances(bidder).unwrap();
    assert_eq!(
        bidder_native_balances,
        vec![coin(INITIAL_BALANCE - 200, NATIVE_DENOM.to_string())]
    );
    // owner gets 200 - 20 - 4
    let owner_native_balances = router.wrap().query_all_balances(owner).unwrap();
    assert_eq!(
        owner_native_balances,
        vec![coin(
            owner_starting_balances[0].amount.u128() + 176,
            NATIVE_DENOM.to_string()
        )]
    );
}
