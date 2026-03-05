use eth2077_execution::eip7702::{
    validate_authorization_list, AuthorizationEntry, AuthorizationError,
};

const SECP256K1_N: [u8; 32] = [
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFE,
    0xBA, 0xAE, 0xDC, 0xE6, 0xAF, 0x48, 0xA0, 0x3B, 0xBF, 0xD2, 0x5E, 0x8C, 0xD0, 0x36, 0x41, 0x41,
];

const SECP256K1_N_DIV_2: [u8; 32] = [
    0x7F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0x5D, 0x57, 0x6E, 0x73, 0x57, 0xA4, 0x50, 0x1D, 0xDF, 0xE9, 0x2F, 0x46, 0x68, 0x1B, 0x20, 0xA0,
];

fn base_authorization() -> AuthorizationEntry {
    AuthorizationEntry {
        chain_id: 1,
        address: [0x11; 20],
        nonce: 42,
        y_parity: 1,
        r: [0x10; 32],
        s: [0x20; 32],
    }
}

fn n_div_2_plus_one() -> [u8; 32] {
    let mut value = SECP256K1_N_DIV_2;
    value[31] += 1;
    value
}

#[test]
fn test_valid_single_authorization() {
    let auth = base_authorization();
    let result = validate_authorization_list(&[auth], 1);
    assert_eq!(result, Ok(()));
}

#[test]
fn test_valid_wildcard_chain_id() {
    let mut auth = base_authorization();
    auth.chain_id = 0;
    let result = validate_authorization_list(&[auth], 7777);
    assert_eq!(result, Ok(()));
}

#[test]
fn test_chain_id_mismatch() {
    let mut auth = base_authorization();
    auth.chain_id = 2;
    let err = validate_authorization_list(&[auth], 1).expect_err("chain mismatch should fail");
    assert_eq!(
        err,
        vec![AuthorizationError::InvalidChainId {
            index: 0,
            chain_id: 2,
            expected: 1
        }]
    );
}

#[test]
fn test_invalid_y_parity() {
    let mut auth = base_authorization();
    auth.y_parity = 2;
    let err = validate_authorization_list(&[auth], 1).expect_err("invalid y_parity should fail");
    assert_eq!(
        err,
        vec![AuthorizationError::InvalidYParity { index: 0, value: 2 }]
    );
}

#[test]
fn test_zero_address_rejected() {
    let mut auth = base_authorization();
    auth.address = [0u8; 20];
    let err = validate_authorization_list(&[auth], 1).expect_err("zero address should fail");
    assert_eq!(err, vec![AuthorizationError::ZeroAddress { index: 0 }]);
}

#[test]
fn test_duplicate_addresses() {
    let mut first = base_authorization();
    first.address = [0xAA; 20];

    let mut second = base_authorization();
    second.address = [0xAA; 20];
    second.nonce = 43;
    second.y_parity = 0;
    second.r = [0x22; 32];
    second.s = [0x33; 32];

    let err =
        validate_authorization_list(&[first, second], 1).expect_err("duplicate addresses fail");

    assert_eq!(
        err,
        vec![AuthorizationError::DuplicateAuthorization {
            index: 1,
            address: [0xAA; 20]
        }]
    );
}

#[test]
fn test_r_at_curve_order_rejected() {
    let mut auth = base_authorization();
    auth.r = SECP256K1_N;
    let err = validate_authorization_list(&[auth], 1).expect_err("r == n should fail");
    assert_eq!(
        err,
        vec![AuthorizationError::SignatureRTooLarge { index: 0 }]
    );
}

#[test]
fn test_s_in_upper_half_rejected() {
    let mut auth = base_authorization();
    auth.s = n_div_2_plus_one();
    let err = validate_authorization_list(&[auth], 1).expect_err("s > n/2 should fail");
    assert_eq!(
        err,
        vec![AuthorizationError::SignatureSInUpperHalf { index: 0 }]
    );
}

#[test]
fn test_valid_low_s() {
    let mut auth = base_authorization();
    auth.s = SECP256K1_N_DIV_2;
    let result = validate_authorization_list(&[auth], 1);
    assert_eq!(result, Ok(()));
}

#[test]
fn test_empty_list_rejected() {
    let err = validate_authorization_list(&[], 1).expect_err("empty list should fail");
    assert_eq!(err, vec![AuthorizationError::EmptyAuthorizationList]);
}

#[test]
fn test_multiple_errors_collected() {
    let bad_zero = AuthorizationEntry {
        chain_id: 9,
        address: [0u8; 20],
        nonce: 1,
        y_parity: 2,
        r: SECP256K1_N,
        s: n_div_2_plus_one(),
    };

    let duplicate_zero = AuthorizationEntry {
        chain_id: 0,
        address: [0u8; 20],
        nonce: 2,
        y_parity: 0,
        r: SECP256K1_N,
        s: n_div_2_plus_one(),
    };

    let err = validate_authorization_list(&[bad_zero, duplicate_zero], 1)
        .expect_err("multiple issues should be collected");

    assert_eq!(err.len(), 9);
    assert!(err.contains(&AuthorizationError::InvalidChainId {
        index: 0,
        chain_id: 9,
        expected: 1
    }));
    assert!(err.contains(&AuthorizationError::InvalidYParity { index: 0, value: 2 }));
    assert!(err.contains(&AuthorizationError::ZeroAddress { index: 0 }));
    assert!(err.contains(&AuthorizationError::ZeroAddress { index: 1 }));
    assert!(err.contains(&AuthorizationError::DuplicateAuthorization {
        index: 1,
        address: [0u8; 20]
    }));
    assert!(err.contains(&AuthorizationError::SignatureRTooLarge { index: 0 }));
    assert!(err.contains(&AuthorizationError::SignatureRTooLarge { index: 1 }));
    assert!(err.contains(&AuthorizationError::SignatureSInUpperHalf { index: 0 }));
    assert!(err.contains(&AuthorizationError::SignatureSInUpperHalf { index: 1 }));
}
