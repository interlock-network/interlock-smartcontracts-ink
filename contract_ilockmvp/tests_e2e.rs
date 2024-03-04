//!
//! INTERLOCK NETWORK MVP SMART CONTRACT END-TO-END TESTS
//!
//! End to end tests are used extensively because using the Openbrush
//! PSP22 framework involves cross-contract invocations under the hood.
//! EG/IE, If I want to reward an Interlocker, this involves an internal
//! call of the OpenBrush PSP22 transfer message. I know of no way to
//! get around this fact for testing besides using end-to-end tests.
//!
//! ##### to setup for e2e testin, run
//!
//! substrate-contracts-node --log info,runtime::contracts=debug 2>&1
//! 
//! ##### after installing by running
//!
//! cargo install contracts-node --git https://github.com/paritytech/substrate-contracts-node.git
//!
//! ##### To view debug prints and assertion failures run test via:
//!  
//! cargo +nightly test --features e2e-tests -- --show-output
//!
//! ##### To view debug for specific method run test via:
//!  
//! cargo nightly+ test <test_function_here> -- --nocapture
//!
//! ! NB ! During test build and runtime, if you ever come across errors
//!        saying 'Metadata artifacts not generated' or 'Once instance
//!        has previously been poisoned', then you need to run `cargo clean`
//!        or delete the `target` directory the build/run from scratch.
//!        OR
//!        Save both the lib.rs file AND this tests_e2e.rs file, then reattempt.
//!

use crate::ilockmvp::*;

#[cfg(all(test, feature = "e2e-tests"))]
use ink_e2e::build_message;

type E2EResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;

use openbrush::{
    contracts:: psp22::psp22_external::PSP22,
    traits::Balance,
};

/// - Test if customized transfer function works correctly.
/// - When transfer from contract owner, circulating supply increases.
/// - When transfer to contract owner, circulating supply decreases
/// and rewards pool increases.
#[ink_e2e::test]
async fn happy_transfer(
    mut client: ink_e2e::Client<C, E>,
) -> E2EResult<()> {

    let alice_account = ink_e2e::account_id(ink_e2e::AccountKeyring::Alice);
    let bob_account = ink_e2e::account_id(ink_e2e::AccountKeyring::Bob);
    let charlie_account = ink_e2e::account_id(ink_e2e::AccountKeyring::Charlie);

    let constructor = ILOCKmvpRef::new_token(
        200_000,
        charlie_account,
        bob_account,
        );
    let contract_acct_id = client
        .instantiate("ilockmvp", &ink_e2e::alice(), constructor, 0, None)
        .await.expect("instantiate failed").account_id;

    // alice rewards 1000 token so charlie can transfer
    let alice_reward_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
        .call(|contract| contract.reward_interlocker(1000, charlie_account.clone()));
    let _reward_response = client
        .call(&ink_e2e::alice(), alice_reward_msg, 0, None).await;

    // transfers 1000 ILOCK from charlie to bob and check for resulting Transfer event
    let charlie_transfer_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
        .call(|contract| contract.transfer(bob_account.clone(), 1000, Vec::new()));
    let transfer_response = client
        .call(&ink_e2e::charlie(), charlie_transfer_msg, 0, None).await.unwrap();

    // filter for transfer event
    let contract_emitted_transfer = transfer_response
        .events
        .iter()
        .find(|event| {
            event
            .as_ref()
            .expect("expected event")
            .event_metadata()
            .event()
            == "ContractEmitted" &&
            String::from_utf8_lossy(
                event.as_ref().expect("bad event").bytes()).to_string()
            .contains("ILOCKmvp::Transfer")
        })
        .expect("Expect ContractEmitted event")
        .unwrap();

    // Decode to the expected event type (skip field_context)
    let transfer_event = contract_emitted_transfer.field_bytes();
    let decoded_transfer =
        <Transfer as scale::Decode>::decode(&mut &transfer_event[35..]).expect("invalid data");

    // Destructor decoded event
    let Transfer { from, to, amount } = decoded_transfer;

    // Assert with the expected value
    assert_eq!(from, Some(charlie_account), "encountered invalid Transfer.from");
    assert_eq!(to, Some(bob_account), "encountered invalid Transfer.to");
    assert_eq!(amount, 1000, "encountered invalid Transfer.amount");

    // checks that bob has expected resulting balance
    let bob_balance_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
        .call(|contract| contract.balance_of(bob_account.clone()));
    let bob_balance = client
        .call_dry_run(&ink_e2e::bob(), &bob_balance_msg, 0, None).await.return_value();
    assert_eq!(0 + 1000, bob_balance);

    // checks that circulating supply increased appropriately
    let total_supply_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
        .call(|contract| contract.total_supply());
    let mut total_supply = client
        .call_dry_run(&ink_e2e::alice(), &total_supply_msg, 0, None).await.return_value();
    assert_eq!(0 + 1000, total_supply);

    // transfers 500 ILOCK from bob to alice
    let bob_transfer_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
        .call(|contract| contract.transfer(alice_account.clone(), 500, Vec::new()));
    let _result = client
        .call(&ink_e2e::bob(), bob_transfer_msg, 0, None).await;

    // checks that circulating supply decreased appropriately
    total_supply = client
        .call_dry_run(&ink_e2e::alice(), &total_supply_msg, 0, None).await.return_value();
    assert_eq!(1000 - 500, total_supply);

    // check that rewards supply increased appropriately
    let rewards_balance_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
        .call(|contract| contract.pool_balance(REWARDS));
    let rewards_balance = client
        .call_dry_run(&ink_e2e::alice(), &rewards_balance_msg, 0, None).await.return_value().unwrap();
    assert_eq!(POOLS[REWARDS as usize].tokens * DECIMALS_POWER10 - 500, rewards_balance);

    // checks that alice has expected resulting balance
    let alice_balance_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
        .call(|contract| contract.balance_of(alice_account.clone()));
    let alice_balance = client
        .call_dry_run(&ink_e2e::alice(), &alice_balance_msg, 0, None).await.return_value();
    assert_eq!(SUPPLY_CAP - 500, alice_balance);

    Ok(())
}

/// - Test if customized transfer function fails correctly.
///
/// - Return
///     InsufficientBalance     - When caller has allowance < amount
///     ZeroRecipientAddress    - when to's address is AccountId::from([0_u8; 32])
///     ZeroSenderAddress       - When caller's address is AccountId::from([0_u8; 32])
///                              (zero address has known private key..however that works)
#[ink_e2e::test]
async fn sad_transfer(
    mut client: ink_e2e::Client<C, E>,
) -> E2EResult<()> {

    Ok(())
}

/// - Test if customized transfer_from function works correctly.
/// - When transfer from contract owner, circulating supply increases.
/// - Transfer and Approval events are emitted.
/// - When transfer to contract owner, circulating supply decreases
/// - When caller transfers, their allowace with from decreases
///   and rewards pool increases
#[ink_e2e::test]
async fn happy_transfer_from(
    mut client: ink_e2e::Client<C, E>,
) -> E2EResult<()> {

    let alice_account = ink_e2e::account_id(ink_e2e::AccountKeyring::Alice);
    let bob_account = ink_e2e::account_id(ink_e2e::AccountKeyring::Bob);
    let charlie_account = ink_e2e::account_id(ink_e2e::AccountKeyring::Charlie);

    let constructor = ILOCKmvpRef::new_token(
        200_000,
        charlie_account,
        bob_account,
        );
    let contract_acct_id = client
        .instantiate("ilockmvp", &ink_e2e::alice(), constructor, 0, None)
        .await.expect("instantiate failed").account_id;

    // alice approves bob 1000 ILOCK
    let alice_approve_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
        .call(|contract| contract.approve(bob_account.clone(), 1000));
    let _approval_result = client
        .call(&ink_e2e::alice(), alice_approve_msg, 0, None).await;

    // bob transfers 1000 ILOCK from alice to charlie
    let bob_transfer_from_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
        .call(|contract| contract.transfer_from(
            alice_account.clone(), charlie_account.clone(), 1000, Vec::new())
        );
    let transfer_from_response = client
        .call(&ink_e2e::bob(), bob_transfer_from_msg, 0, None).await.unwrap();

    // filter for approval event
    let contract_emitted_approval = transfer_from_response
        .events
        .iter()
        .find(|event| {
            event
            .as_ref()
            .expect("expected event")
            .event_metadata()
            .event()
            == "ContractEmitted" &&
            String::from_utf8_lossy(
                event.as_ref().expect("bad event").bytes()).to_string()
           .contains("ILOCKmvp::Approval")
        })
        .expect("Expect ContractEmitted event")
        .unwrap();

    // decode to the expected event type (skip field_context)
    let approval_event = contract_emitted_approval.field_bytes();
    let decoded_approval =
        <Approval as scale::Decode>::decode(&mut &approval_event[35..]).expect("invalid data");

    // destructor decoded eapproval
    let Approval { owner, spender, amount } = decoded_approval;

    // assert with the expected value
    assert_eq!(owner, Some(alice_account), "encountered invalid Approval.owner");
    assert_eq!(spender, Some(bob_account), "encountered invalid Approval.spender");
    assert_eq!(amount, 1000 - 1000, "encountered invalid Approval.amount");

    // filter for transfer event
    let contract_emitted_transfer = transfer_from_response
        .events
        .iter()
        .find(|event| {
            event
            .as_ref()
            .expect("expected event")
            .event_metadata()
            .event()
            == "ContractEmitted" &&
            String::from_utf8_lossy(
            event.as_ref().expect("bad event").bytes()).to_string()
               .contains("ILOCKmvp::Transfer")
        })
        .expect("Expect ContractEmitted event")
        .unwrap();
    
    // decode to the expected event type (skip field_context)
    let transfer_event = contract_emitted_transfer.field_bytes();
    let decoded_transfer =
        <Transfer as scale::Decode>::decode(&mut &transfer_event[35..]).expect("invalid data");

    // destructor decoded transfer
    let Transfer { from, to, amount } = decoded_transfer;
    
    // assert with the expected value
    assert_eq!(from, Some(alice_account), "encountered invalid Transfer.from");
    assert_eq!(to, Some(charlie_account), "encountered invalid Transfer.to");
    assert_eq!(amount, 1000, "encountered invalid Transfer.amount");

    // checks that charlie has expected resulting balance
    let charlie_balance_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
        .call(|contract| contract.balance_of(charlie_account.clone()));
    let charlie_balance = client
        .call_dry_run(&ink_e2e::charlie(), &charlie_balance_msg, 0, None).await.return_value();
    assert_eq!(0 + 1000, charlie_balance);

    // checks that circulating supply increased appropriately
    let total_supply_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
        .call(|contract| contract.total_supply());
    let mut total_supply = client
        .call_dry_run(&ink_e2e::alice(), &total_supply_msg, 0, None).await.return_value();
    assert_eq!(0 + 1000, total_supply);

    // checks that bob's allowance decreased appropriately
    let bob_allowance_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
        .call(|contract| contract.allowance(alice_account.clone(), bob_account.clone()));
    let bob_allowance = client
        .call_dry_run(&ink_e2e::alice(), &bob_allowance_msg, 0, None).await.return_value();
    assert_eq!(1000 - 1000, bob_allowance);

    // charlie approves bob 1000 ILOCK
    let charlie_approve_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
        .call(|contract| contract.approve(bob_account.clone(), 1000));
    let _approval_result = client
        .call(&ink_e2e::charlie(), charlie_approve_msg, 0, None).await;

    // bob transfers 1000 ILOCK from charlie to alice
    let bob_transfer_from_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
        .call(|contract| contract.transfer_from(
            charlie_account.clone(), alice_account.clone(), 1000, Vec::new()));
    let _transfer_from_result = client
        .call(&ink_e2e::bob(), bob_transfer_from_msg, 0, None).await;

    // checks that circulating supply decreased appropriately
    total_supply = client
        .call_dry_run(&ink_e2e::alice(), &total_supply_msg, 0, None).await.return_value();
    assert_eq!(1000 - 1000, total_supply);

    // check that rewards supply increased appropriately
    let rewards_balance_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
        .call(|contract| contract.pool_balance(REWARDS));
    let rewards_balance = client
        .call_dry_run(&ink_e2e::alice(), &rewards_balance_msg, 0, None).await.return_value().unwrap();
    assert_eq!(POOLS[REWARDS as usize].tokens * DECIMALS_POWER10 + 1000, rewards_balance);

    Ok(())
}

/// - Test if customized transfer_from function fails correctly.
///
/// - Return
///     InsufficientBalance     - When caller has allowance < amount
///     InsufficientAllowance   - When caller specs amount > from's balance
///     ZeroRecipientAddress    - when to's address is AccountId::from([0_u8; 32])
///     ZeroSenderAddress       - When from's address is AccountId::from([0_u8; 32])
#[ink_e2e::test]
async fn sad_transfer_from(
    mut client: ink_e2e::Client<C, E>,
) -> E2EResult<()> {

    Ok(())
}

/// - Test if token distribution works as intended per vesting schedule.
/// - Cycle through entire vesting period.
/// - Includes optional print table for inspection
/// - Includes register_stakeholder().
/// - Includes distribute_tokens().
#[ink_e2e::test]
async fn happy_distribute_tokens(
    mut client: ink_e2e::Client<C, E>,
) -> E2EResult<()> {

    let bob_account = ink_e2e::account_id(ink_e2e::AccountKeyring::Bob);
    let charlie_account = ink_e2e::account_id(ink_e2e::AccountKeyring::Charlie);

    // fire up contract
    let constructor = ILOCKmvpRef::new_token(
        200_000,
        charlie_account,
        bob_account,
        );
    let contract_acct_id = client
        .instantiate("ilockmvp", &ink_e2e::alice(), constructor, 0, None)
        .await.expect("instantiate failed").account_id;

    // register accounts
    let alice_account = ink_e2e::account_id(ink_e2e::AccountKeyring::Alice);
    let stakeholder_account = ink_e2e::account_id(ink_e2e::AccountKeyring::Bob);
    let stakeholder_share = 1_000_000_000;
    let pool_size = POOLS[TEAM as usize].tokens * DECIMALS_POWER10;

    // register stakeholder
    let register_stakeholder_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
        .call(|contract| contract.register_stakeholder(
            stakeholder_account.clone(), stakeholder_share, TEAM, false));
    let _register_stakeholder_result = client
        .call(&ink_e2e::alice(), register_stakeholder_msg, 0, None).await;

    let cliff = POOLS[TEAM as usize].cliffs;
    let vests = POOLS[TEAM as usize].vests;
    let schedule_end = vests + cliff - 1;
    let schedule_period = vests;
    let payout = 1_000_000_000 / vests as Balance; // 27_777_777
    let last_payout = payout + 1_000_000_000 % vests as Balance; // 27_777_805

    // check stakeholder_data()
    let stake_data_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
        .call(|contract| contract.get_stakes(stakeholder_account.clone()));
    let _stake_data = client
        .call_dry_run(&ink_e2e::alice(), &stake_data_msg, 0, None).await.return_value();
    /*
    let stake_payout_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
        .call(|contract| contract.get_stakes_payamount(stakeholder_account.clone()));
    let stake_payout = client
        .call_dry_run(&ink_e2e::alice(), &stake_payout_msg, 0, None).await.return_value().unwrap().first().unwrap();
    assert_eq!(stake_data.share, stakeholder_share);
    assert_eq!(*stake_payout, payout);
*/
    // iterate through one vesting schedule
    for month in 0..(schedule_end + 2) {

        if month >= cliff && month <= schedule_end {

        let distribute_tokens_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
            .call(|contract| contract.distribute_tokens(stakeholder_account.clone(), TEAM));
        let _distribute_tokens_result = client
            .call(&ink_e2e::alice(), distribute_tokens_msg, 0, None).await;
        }

        let stake_data_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
            .call(|contract| contract.get_stakes(stakeholder_account.clone()));
        let stake_data = client
            .call_dry_run(&ink_e2e::alice(), &stake_data_msg, 0, None).await.return_value().expect("bad call").first().unwrap().clone();
        let stake_paid = stake_data.clone().paid;
     //       .call_dry_run(&ink_e2e::alice(), &stakeholder_data_msg, 0, None)
       //         .await.return_value().0.paid;

        let stakeholder_balance_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
            .call(|contract| contract.balance_of(stakeholder_account.clone()));
        let stakeholder_balance = client
            .call_dry_run(&ink_e2e::alice(), &stakeholder_balance_msg.clone(), 0, None)
                .await.return_value();

        let pool_balance_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
            .call(|contract| contract.pool_balance(TEAM));
        let pool_balance = client
            .call_dry_run(&ink_e2e::alice(), &pool_balance_msg, 0, None)
                .await.return_value();

        let owner_balance_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
            .call(|contract| contract.balance_of(alice_account.clone()));
        let owner_balance = client
            .call_dry_run(&ink_e2e::alice(), &owner_balance_msg.clone(), 0, None)
                .await.return_value();

        let increment_month_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
            .call(|contract| contract.TESTING_increment_month());
        let _increment_month_result = client
            .call(&ink_e2e::alice(), increment_month_msg, 0, None).await;

        /* // visual proof of workee
        println!("{:?}", month_result);
        println!("{:?}", stakeholder_paid);
        println!("{:?}", stakeholder_balance);
        println!("{:?}", pool_balance);
        println!("{:?}", owner_balance);
        */
        if month < cliff {

            assert_eq!(stake_paid, 0);
            assert_eq!(stakeholder_balance, 0);
            assert_eq!(owner_balance, SUPPLY_CAP);
            assert_eq!(pool_balance.unwrap(), pool_size);

        } else if month >= cliff && month < schedule_end {

            assert_eq!(stake_paid, (month - cliff + 1) as Balance * payout);
            assert_eq!(stakeholder_balance, (month - cliff + 1) as Balance * payout);
            assert_eq!(owner_balance, SUPPLY_CAP - (month - cliff + 1) as Balance * payout);
            assert_eq!(pool_balance.unwrap(), pool_size - (month - cliff + 1) as Balance * payout);

        } else if month >= schedule_end {

            assert_eq!(stake_paid, (schedule_period - 1) as Balance * payout + last_payout);
            assert_eq!(stakeholder_balance, (schedule_period - 1) as Balance * payout + last_payout);
            assert_eq!(owner_balance,
               SUPPLY_CAP - (schedule_period - 1) as Balance * payout - last_payout);
            assert_eq!(pool_balance.unwrap(),
               pool_size - (schedule_period - 1) as Balance * payout - last_payout);
        }
    }
    Ok(())
}

/// - Check to make sure distribute_tokens fails as expected.
///
/// - Return
///     CallerNotOwner          - When caller does not own contract
///     StakeholderNotFound     - when stakeholder specified isn't registered
///     CliffNotPassed          - when pool's vesting cliff isn't passed
///     StakeholderSharePaid    - when stakeholder has already been paid entire share
///     PayoutTooEarly          - when next month's payment isn't ready
#[ink_e2e::test]
async fn sad_distribute_tokens(
    mut client: ink_e2e::Client<C, E>,
) -> E2EResult<()> {

    Ok(())
}

/// - Check to make sure payout_tokens works as expected.
/// - Checks PARTNERS, COMMUNITY, and PUBLIC pools.
/// - Checks resulting balances for three pools and recipients.
#[ink_e2e::test]
async fn happy_payout_tokens(
    mut client: ink_e2e::Client<C, E>,
) -> E2EResult<()> {

    let alice_account = ink_e2e::account_id(ink_e2e::AccountKeyring::Alice);
    let bob_account = ink_e2e::account_id(ink_e2e::AccountKeyring::Bob);
    let charlie_account = ink_e2e::account_id(ink_e2e::AccountKeyring::Charlie);

    let constructor = ILOCKmvpRef::new_token(
        200_000,
        charlie_account,
        bob_account,
        );
    let contract_acct_id = client
        .instantiate("ilockmvp", &ink_e2e::alice(), constructor, 0, None)
            .await.expect("instantiate failed").account_id;

    // register stakeholder
    let register_stakeholder_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
        .call(|contract| contract.register_stakeholder(
            bob_account.clone(), 1_000_000, PARTNERS, false));
    let _register_stakeholder_result = client
        .call(&ink_e2e::alice(), register_stakeholder_msg, 0, None).await;

    // register stakeholder
    let register_stakeholder_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
        .call(|contract| contract.register_stakeholder(
            bob_account.clone(), 1_000_000, COMMUNITY, false));
    let _register_stakeholder_result = client
        .call(&ink_e2e::alice(), register_stakeholder_msg, 0, None).await;

    // register stakeholder
    let register_stakeholder_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
        .call(|contract| contract.register_stakeholder(
            bob_account.clone(), 1_000_000, PUBLIC, false));
    let _register_stakeholder_result = client
        .call(&ink_e2e::alice(), register_stakeholder_msg, 0, None).await;

    // messages the pay from various pools
    let partners_pay_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
        .call(|contract| contract.payout_tokens(
            bob_account.clone(), 1000, "PARTNERS".to_string()));
    let whitelist_pay_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
        .call(|contract| contract.payout_tokens(
            bob_account.clone(), 1000, "COMMUNITY".to_string()));
    let publicsale_pay_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
        .call(|contract| contract.payout_tokens(
            bob_account.clone(), 1000, "PUBLIC".to_string()));

    // alice pays 1000 ILOCK to bob from PARTNERS pool
    let _partners_pay_result = client
        .call(&ink_e2e::alice(), partners_pay_msg, 0, None).await;

    // checks that alice has expected resulting balance
    let alice_balance_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
        .call(|contract| contract.balance_of(alice_account.clone()));
    let mut alice_balance = client
        .call_dry_run(&ink_e2e::alice(), &alice_balance_msg, 0, None).await.return_value();
    assert_eq!(SUPPLY_CAP - 1000, alice_balance);

    // checks that bob has expected resulting balance
    let bob_balance_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
        .call(|contract| contract.balance_of(bob_account.clone()));
    let mut bob_balance = client
        .call_dry_run(&ink_e2e::alice(), &bob_balance_msg, 0, None).await.return_value();
    assert_eq!(0 + 1000, bob_balance);

    // checks that pool has expected resulting balance
    let mut pool_balance_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
        .call(|contract| contract.pool_balance(PARTNERS));
    let mut pool_balance = client
        .call_dry_run(&ink_e2e::alice(), &pool_balance_msg, 0, None).await.return_value().unwrap();
    assert_eq!(POOLS[PARTNERS as usize].tokens * DECIMALS_POWER10 - 1000, pool_balance);

    // alice pays 1000 ILOCK to bob from COMMUNITY pool
    let _whitelist_pay_result = client
        .call(&ink_e2e::alice(), whitelist_pay_msg, 0, None).await;

    // checks that alice has expected resulting balance
    alice_balance = client
        .call_dry_run(&ink_e2e::alice(), &alice_balance_msg, 0, None).await.return_value();
    assert_eq!(SUPPLY_CAP - 1000 - 1000, alice_balance);

    // checks that bob has expected resulting balance
    bob_balance = client
        .call_dry_run(&ink_e2e::alice(), &bob_balance_msg, 0, None).await.return_value();
    assert_eq!(0 + 1000 + 1000, bob_balance);

    // checks that pool has expected resulting balance
    pool_balance_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
        .call(|contract| contract.pool_balance(COMMUNITY));
    pool_balance = client
        .call_dry_run(&ink_e2e::alice(), &pool_balance_msg, 0, None).await.return_value().unwrap();
    assert_eq!(POOLS[COMMUNITY as usize].tokens * DECIMALS_POWER10 - 1000, pool_balance);

    // alice pays 1000 ILOCK to bob from PUBLIC pool
    let _publicsale_pay_result = client
        .call(&ink_e2e::alice(), publicsale_pay_msg, 0, None).await;

    // checks that alice has expected resulting balance
    alice_balance = client
        .call_dry_run(&ink_e2e::alice(), &alice_balance_msg, 0, None).await.return_value();
    assert_eq!(SUPPLY_CAP - 1000 - 1000 - 1000, alice_balance);

    // checks that bob has expected resulting balance
    bob_balance = client
        .call_dry_run(&ink_e2e::alice(), &bob_balance_msg, 0, None).await.return_value();
    assert_eq!(0 + 1000 + 1000 + 1000, bob_balance);

    // checks that pool has expected resulting balance
    pool_balance_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
        .call(|contract| contract.pool_balance(PUBLIC));
    pool_balance = client
        .call_dry_run(&ink_e2e::alice(), &pool_balance_msg, 0, None).await.return_value().unwrap();
    assert_eq!(POOLS[PUBLIC as usize].tokens * DECIMALS_POWER10 - 1000, pool_balance);
    
    Ok(())
}

/// - Checks to make sure payout_tokens function fails as expected.
///
/// - Return
///     CallerNotOwner          - when caller does not own contract
///     InvalidPool             - when pool isn't (PARTNERS|COMMUNITY|PUBLIC)
///     PaymentTooLarge         - when specified payment amount is more than pool
#[ink_e2e::test]
async fn sad_payout_tokens(
    mut client: ink_e2e::Client<C, E>,
) -> E2EResult<()> {

    Ok(())
}

/// - Test if rewarding functionality works.
/// - Update rewardedtotal.
/// - Transfer reward amount from rewards pool to Interlocker.
/// - Update individual rewardedinterlockertotal
/// - Emit reward event.
/// - Return new interlocker rewarded total.
/// - Test that rewarded_total() works.
/// - Test that rewarded_interlocker_total() works.
#[ink_e2e::test]
async fn happy_reward_interlocker(
    mut client: ink_e2e::Client<C, E>,
) -> E2EResult<()> {

    let alice_account = ink_e2e::account_id(ink_e2e::AccountKeyring::Alice);
    let bob_account = ink_e2e::account_id(ink_e2e::AccountKeyring::Bob);
    let charlie_account = ink_e2e::account_id(ink_e2e::AccountKeyring::Charlie);

    let constructor = ILOCKmvpRef::new_token(
        200_000,
        charlie_account,
        bob_account,
        );
    let contract_acct_id = client
        .instantiate("ilockmvp", &ink_e2e::alice(), constructor, 0, None)
            .await.expect("instantiate failed").account_id;

    // alice rewards bob the happy interlocker 1000 ILOCK
    let alice_reward_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
        .call(|contract| contract.reward_interlocker(1000, bob_account.clone()));
    let reward_response = client
        .call(&ink_e2e::alice(), alice_reward_msg, 0, None).await.unwrap();

    // filter for reward event
    let contract_emitted_reward = reward_response
        .events
        .iter()
        .find(|event| {
            event
            .as_ref()
            .expect("expected event")
            .event_metadata()
            .event()
            == "ContractEmitted" &&
            String::from_utf8_lossy(
                event.as_ref().expect("bad event").bytes()).to_string()
           .contains("ILOCKmvp::Reward")
        })
        .expect("Expect ContractEmitted event")
        .unwrap();

    // decode to the expected event type (skip field_context)
    let reward_event = contract_emitted_reward.field_bytes();
    let decoded_reward =
        <Reward as scale::Decode>::decode(&mut &reward_event[34..]).expect("invalid data");

    // destructor decoded transfer
    let Reward { to, amount } = decoded_reward;

    // assert with the expected value
    assert_eq!(to, Some(bob_account), "encountered invalid Reward.to");
    assert_eq!(amount, 1000, "encountered invalid Reward.amount");

    // checks that alice has expected resulting balance
    let alice_balance_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
        .call(|contract| contract.balance_of(alice_account.clone()));
    let alice_balance = client
        .call_dry_run(&ink_e2e::alice(), &alice_balance_msg, 0, None).await.return_value();
    assert_eq!(SUPPLY_CAP - 1000, alice_balance);

    // checks that pool has expected resulting balance
    let pool_balance_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
        .call(|contract| contract.pool_balance(REWARDS));
    let pool_balance = client
        .call_dry_run(&ink_e2e::alice(), &pool_balance_msg, 0, None).await.return_value().unwrap();
    assert_eq!(POOLS[REWARDS as usize].tokens * DECIMALS_POWER10 - 1000, pool_balance);

    // checks that bob has expected resulting balance
    let bob_balance_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
        .call(|contract| contract.balance_of(bob_account.clone()));
    let bob_balance = client
        .call_dry_run(&ink_e2e::alice(), &bob_balance_msg, 0, None).await.return_value();
    assert_eq!(0 + 1000, bob_balance);

    // checks that circulating supply was properly incremented
    let total_supply_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
        .call(|contract| contract.total_supply());
    let total_supply = client
        .call_dry_run(&ink_e2e::alice(), &total_supply_msg, 0, None).await.return_value();
    assert_eq!(0 + 1000, total_supply);

    // checks that total rewarded (overall) is correct
    let total_rewarded_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
        .call(|contract| contract.rewarded_total());
    let total_rewarded = client
        .call_dry_run(&ink_e2e::alice(), &total_rewarded_msg, 0, None).await.return_value();
    assert_eq!(0 + 1000, total_rewarded);

    // checks that total rewarded (to interlocker) is correct
    let total_rewarded_interlocker_msg = build_message::<ILOCKmvpRef>(contract_acct_id.clone())
        .call(|contract| contract.rewarded_interlocker_total(bob_account.clone()));
    let total_rewarded_interlocker = client
        .call_dry_run(&ink_e2e::alice(), &total_rewarded_interlocker_msg, 0, None).await.return_value().unwrap();
    assert_eq!(0 + 1000, total_rewarded_interlocker);
    
    Ok(())
}

/// - Test if rewarding functionality fails correctly.
///
/// - Return
///     CallerNotOwner      - when caller does not own contract
///     PaymentTooLarge     - when arithmetic over or underflows
///
/// ... maybe check the over/underflows?
#[ink_e2e::test]
async fn sad_reward_interlocker(
    mut client: ink_e2e::Client<C, E>,
) -> E2EResult<()> {

    Ok(())
}
