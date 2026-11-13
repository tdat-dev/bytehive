use super::*;

/// The shared vector: remote-web/src/store/crypto.test.ts checks the same numbers.
/// Few rounds so the test is quick; the code path is the same as with ROUNDS.
const PASS: &str = "correct horse battery staple";
const SALT: &[u8] = b"bytehive-salt-01";
const ROUNDS_TEST: u32 = 1000;

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[test]
fn derives_the_shared_vector() {
    let k = derive(PASS, SALT, ROUNDS_TEST);
    assert_eq!(hex(&k.enc), ENC_HEX);
    assert_eq!(hex(&k.id), ID_HEX);
}

#[test]
fn seals_the_shared_vector_with_a_fixed_nonce() {
    let k = derive(PASS, SALT, ROUNDS_TEST);
    let nonce = [7u8; 12];
    assert_eq!(seal_with(&k, "abc/0", b"hello", &nonce).unwrap(), SEALED_HELLO);
}

#[test]
fn names_a_session_the_shared_way() {
    let k = derive(PASS, SALT, ROUNDS_TEST);
    assert_eq!(sid(&k, "m:desk", "claude", "s-1"), SID);
}

#[test]
fn opens_what_it_sealed_only_in_the_same_slot_with_the_same_key() {
    let k = derive(PASS, SALT, ROUNDS_TEST);
    let s = seal(&k, "abc/0", b"hello").unwrap();
    assert_eq!(open(&k, "abc/0", &s).as_deref(), Some(&b"hello"[..]));
    assert_eq!(open(&k, "abc/1", &s), None, "moved to another slot");
    assert_eq!(open(&derive("wrong", SALT, ROUNDS_TEST), "abc/0", &s), None, "wrong passphrase");
    assert_eq!(open(&k, "abc/0", "garbage!"), None);
}

#[test]
fn tells_a_wrong_passphrase_by_the_check_blob() {
    let k = derive(PASS, SALT, ROUNDS_TEST);
    let c = check(&k).unwrap();
    assert!(checks(&k, &c));
    assert!(!checks(&derive("wrong", SALT, ROUNDS_TEST), &c));
}

#[test]
fn never_seals_the_same_text_the_same_way_twice() {
    let k = derive(PASS, SALT, ROUNDS_TEST);
    assert_ne!(seal(&k, "a/0", b"x").unwrap(), seal(&k, "a/0", b"x").unwrap());
}

// Computed with WebCrypto (the browser's side), not with this code; the web test pins the same.
const ENC_HEX: &str = "1880b42ad33347acbb9290e909ae7c12b91d2d4c4c67133a9c143190ff62ff38";
const ID_HEX: &str = "2533bbe03b0b31c2beba0914b1166d5872f53052c452de13e7bbff3798832aa2";
const SEALED_HELLO: &str = "AQcHBwcHBwcHBwcHB8dvBfsKqrtJPA98h8dzr683MFLazg";
const SID: &str = "30d8d447732f597f7ecef755629f804c";
