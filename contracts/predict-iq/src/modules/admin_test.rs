#![cfg(test)]
use super::admin::*;
use crate::errors::ErrorCode;
use crate::{PredictIQ, PredictIQClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

fn setup() -> (Env, Address) {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, PredictIQ);
    (e, contract_id)
}

#[test]
fn test_set_and_get_admin() {
    let (e, contract_id) = setup();
    let admin = Address::generate(&e);

    e.as_contract(&contract_id, || {
        set_admin(&e, admin.clone());
        let stored_admin = get_admin(&e).unwrap();
        assert_eq!(stored_admin, admin);
    });
}

#[test]
fn test_require_admin_success() {
    let (e, contract_id) = setup();
    let admin = Address::generate(&e);

    e.as_contract(&contract_id, || {
        set_admin(&e, admin.clone());
        let result = require_admin(&e);
        assert!(result.is_ok());
    });
}

#[test]
fn test_require_admin_not_set() {
    let (e, contract_id) = setup();

    e.as_contract(&contract_id, || {
        let result = require_admin(&e);
        assert_eq!(result, Err(ErrorCode::NotAuthorized));
    });
}

#[test]
fn test_set_and_get_market_admin() {
    let (e, contract_id) = setup();
    let admin = Address::generate(&e);
    let market_admin = Address::generate(&e);

    e.as_contract(&contract_id, || {
        set_admin(&e, admin.clone());
        set_market_admin(&e, market_admin.clone()).unwrap();
        let stored_market_admin = get_market_admin(&e).unwrap();
        assert_eq!(stored_market_admin, market_admin);
    });
}

#[test]
fn test_require_market_admin_success() {
    let (e, contract_id) = setup();
    let admin = Address::generate(&e);
    let market_admin = Address::generate(&e);

    e.as_contract(&contract_id, || {
        set_admin(&e, admin.clone());
        set_market_admin(&e, market_admin.clone()).unwrap();
        let result = require_market_admin(&e);
        assert!(result.is_ok());
    });
}

#[test]
fn test_require_market_admin_not_set() {
    let (e, contract_id) = setup();

    e.as_contract(&contract_id, || {
        let result = require_market_admin(&e);
        assert_eq!(result, Err(ErrorCode::NotAuthorized));
    });
}

#[test]
fn test_market_admin_cannot_be_set_by_non_admin() {
    let (e, contract_id) = setup();
    let admin = Address::generate(&e);
    let market_admin = Address::generate(&e);
    let non_admin = Address::generate(&e);

    e.as_contract(&contract_id, || {
        set_admin(&e, admin.clone());
        // Try to set market admin as non-admin (should fail)
        e.mock_all_auths_allowing_non_root_auth();
        e.set_auths(&[(non_admin.clone(), soroban_sdk::testutils::AuthorizedInvocation {
            function: soroban_sdk::testutils::AuthorizedFunction::Contract((
                e.current_contract_address(),
                soroban_sdk::Symbol::new(&e, "set_market_admin"),
                soroban_sdk::vec![&e, soroban_sdk::Val::from_void()],
            )),
            sub_invocations: soroban_sdk::vec![&e],
        })]);
        let result = set_market_admin(&e, market_admin.clone());
        assert_eq!(result, Err(ErrorCode::NotAuthorized));
    });
}

#[test]
fn test_set_and_get_fee_admin() {
    let (e, contract_id) = setup();
    let admin = Address::generate(&e);
    let fee_admin = Address::generate(&e);

    e.as_contract(&contract_id, || {
        set_admin(&e, admin.clone());
        set_fee_admin(&e, fee_admin.clone()).unwrap();
        let stored_fee_admin = get_fee_admin(&e).unwrap();
        assert_eq!(stored_fee_admin, fee_admin);
    });
}

#[test]
fn test_require_fee_admin_success() {
    let (e, contract_id) = setup();
    let admin = Address::generate(&e);
    let fee_admin = Address::generate(&e);

    e.as_contract(&contract_id, || {
        set_admin(&e, admin.clone());
        set_fee_admin(&e, fee_admin.clone()).unwrap();
        let result = require_fee_admin(&e);
        assert!(result.is_ok());
    });
}

#[test]
fn test_require_fee_admin_not_set() {
    let (e, contract_id) = setup();

    e.as_contract(&contract_id, || {
        let result = require_fee_admin(&e);
        assert_eq!(result, Err(ErrorCode::NotAuthorized));
    });
}

#[test]
fn test_fee_admin_cannot_be_set_by_non_admin() {
    let (e, contract_id) = setup();
    let admin = Address::generate(&e);
    let fee_admin = Address::generate(&e);
    let non_admin = Address::generate(&e);

    e.as_contract(&contract_id, || {
        set_admin(&e, admin.clone());
        // Try to set fee admin as non-admin (should fail)
        e.mock_all_auths_allowing_non_root_auth();
        e.set_auths(&[(non_admin.clone(), soroban_sdk::testutils::AuthorizedInvocation {
            function: soroban_sdk::testutils::AuthorizedFunction::Contract((
                e.current_contract_address(),
                soroban_sdk::Symbol::new(&e, "set_fee_admin"),
                soroban_sdk::vec![&e, soroban_sdk::Val::from_void()],
            )),
            sub_invocations: soroban_sdk::vec![&e],
        })]);
        let result = set_fee_admin(&e, fee_admin.clone());
        assert_eq!(result, Err(ErrorCode::NotAuthorized));
    });
}

#[test]
fn test_set_and_get_guardian() {
    let (e, contract_id) = setup();
    let admin = Address::generate(&e);
    let guardian = Address::generate(&e);

    e.as_contract(&contract_id, || {
        set_admin(&e, admin.clone());
        set_guardian(&e, guardian.clone()).unwrap();
        let stored_guardian = get_guardian(&e).unwrap();
        assert_eq!(stored_guardian, guardian);
    });
}

#[test]
fn test_require_guardian_success() {
    let (e, contract_id) = setup();
    let admin = Address::generate(&e);
    let guardian = Address::generate(&e);

    e.as_contract(&contract_id, || {
        set_admin(&e, admin.clone());
        set_guardian(&e, guardian.clone()).unwrap();
        let result = require_guardian(&e);
        assert!(result.is_ok());
    });
}

#[test]
fn test_require_guardian_not_set() {
    let (e, contract_id) = setup();

    e.as_contract(&contract_id, || {
        let result = require_guardian(&e);
        assert_eq!(result, Err(ErrorCode::NotAuthorized));
    });
}

#[test]
fn test_admin_can_change_guardian() {
    let (e, contract_id) = setup();
    let admin = Address::generate(&e);
    let guardian1 = Address::generate(&e);
    let guardian2 = Address::generate(&e);

    e.as_contract(&contract_id, || {
        set_admin(&e, admin.clone());
        set_guardian(&e, guardian1.clone()).unwrap();
        assert_eq!(get_guardian(&e).unwrap(), guardian1);
        set_guardian(&e, guardian2.clone()).unwrap();
        assert_eq!(get_guardian(&e).unwrap(), guardian2);
    });
}

#[test]
fn test_admin_and_guardian_are_independent() {
    let (e, contract_id) = setup();
    let admin = Address::generate(&e);
    let guardian = Address::generate(&e);

    e.as_contract(&contract_id, || {
        set_admin(&e, admin.clone());
        set_guardian(&e, guardian.clone()).unwrap();
        assert_eq!(get_admin(&e).unwrap(), admin);
        assert_eq!(get_guardian(&e).unwrap(), guardian);
        assert_ne!(admin, guardian);
    });
}

#[test]
fn test_role_segregation_market_admin_vs_fee_admin() {
    let (e, contract_id) = setup();
    let admin = Address::generate(&e);
    let market_admin = Address::generate(&e);
    let fee_admin = Address::generate(&e);

    e.as_contract(&contract_id, || {
        set_admin(&e, admin.clone());
        set_market_admin(&e, market_admin.clone()).unwrap();
        set_fee_admin(&e, fee_admin.clone()).unwrap();

        // Market admin can access market functions
        assert!(require_market_admin(&e).is_ok());

        // Fee admin cannot access market functions
        e.mock_all_auths_allowing_non_root_auth();
        e.set_auths(&[(fee_admin.clone(), soroban_sdk::testutils::AuthorizedInvocation {
            function: soroban_sdk::testutils::AuthorizedFunction::Contract((
                e.current_contract_address(),
                soroban_sdk::Symbol::new(&e, "require_market_admin"),
                soroban_sdk::vec![&e],
            )),
            sub_invocations: soroban_sdk::vec![&e],
        })]);
        assert_eq!(require_market_admin(&e), Err(ErrorCode::NotAuthorized));
    });
}

#[test]
fn test_role_segregation_fee_admin_vs_market_admin() {
    let (e, contract_id) = setup();
    let admin = Address::generate(&e);
    let market_admin = Address::generate(&e);
    let fee_admin = Address::generate(&e);

    e.as_contract(&contract_id, || {
        set_admin(&e, admin.clone());
        set_market_admin(&e, market_admin.clone()).unwrap();
        set_fee_admin(&e, fee_admin.clone()).unwrap();

        // Fee admin can access fee functions
        assert!(require_fee_admin(&e).is_ok());

        // Market admin cannot access fee functions
        e.mock_all_auths_allowing_non_root_auth();
        e.set_auths(&[(market_admin.clone(), soroban_sdk::testutils::AuthorizedInvocation {
            function: soroban_sdk::testutils::AuthorizedFunction::Contract((
                e.current_contract_address(),
                soroban_sdk::Symbol::new(&e, "require_fee_admin"),
                soroban_sdk::vec![&e],
            )),
            sub_invocations: soroban_sdk::vec![&e],
        })]);
        assert_eq!(require_fee_admin(&e), Err(ErrorCode::NotAuthorized));
    });
}
