//! bytehive-noise for the browser.

use bytehive_noise as noise;
use wasm_bindgen::prelude::*;

fn fail(e: noise::Error) -> JsError {
    JsError::new(&format!("{e:?}"))
}
fn key(k: &[u8]) -> Result<[u8; 32], JsError> {
    k.try_into().map_err(|_| JsError::new("a key is 32 bytes"))
}

#[wasm_bindgen(getter_with_clone)]
pub struct KeyPair {
    pub private: Vec<u8>,
    pub public: Vec<u8>,
}

#[wasm_bindgen(js_name = generateKeys)]
pub fn generate_keys() -> Result<KeyPair, JsError> {
    let k = noise::generate_keys().map_err(fail)?;
    Ok(KeyPair { private: k.private, public: k.public })
}

#[wasm_bindgen(js_name = newCode)]
pub fn new_code() -> String {
    noise::new_code()
}

#[wasm_bindgen]
pub fn seal(k: &[u8], aad: &[u8], plain: &[u8]) -> Result<Vec<u8>, JsError> {
    noise::seal(&key(k)?, aad, plain).map_err(fail)
}

#[wasm_bindgen]
pub fn open(k: &[u8], aad: &[u8], sealed: &[u8]) -> Result<Vec<u8>, JsError> {
    noise::open(&key(k)?, aad, sealed).map_err(fail)
}

/// One side of a session, always the initiator: a device calls the machine.
///
/// `remoteKey` is the machine's key as soon as its reply (message 2) has been
/// read, so a device can compare it with the key it pinned before it sends
/// message 3 and reveals its own identity. In XXpsk3 nothing yet proves the
/// peer knew the pairing code: pin a key only once `confirmed` is true, which
/// happens at the first successful `decrypt`. The machine must therefore send a
/// first transport message right after the handshake.
#[wasm_bindgen]
pub struct Session {
    handshake: Option<noise::Handshake>,
    transport: Option<noise::Transport>,
    remote: Option<Vec<u8>>,
    confirmed: bool,
}

#[wasm_bindgen]
impl Session {
    /// `code` is the pairing code, or undefined for a machine already paired.
    #[wasm_bindgen(constructor)]
    pub fn new(private: &[u8], code: Option<String>) -> Result<Session, JsError> {
        let psk = code.map(|c| noise::psk_from_code(&c));
        let h = noise::Handshake::new(noise::Role::Initiator, private, psk).map_err(fail)?;
        Ok(Session { handshake: Some(h), transport: None, remote: None, confirmed: false })
    }
    pub fn write(&mut self, payload: &[u8]) -> Result<Vec<u8>, JsError> {
        let h = self.handshake.as_mut().ok_or_else(|| JsError::new("handshake over"))?;
        let out = h.write(payload).map_err(fail)?;
        self.settle()?;
        Ok(out)
    }
    pub fn read(&mut self, msg: &[u8]) -> Result<Vec<u8>, JsError> {
        let h = self.handshake.as_mut().ok_or_else(|| JsError::new("handshake over"))?;
        let out = h.read(msg).map_err(fail)?;
        self.settle()?;
        Ok(out)
    }
    fn settle(&mut self) -> Result<(), JsError> {
        if self.handshake.as_ref().is_some_and(|h| h.finished()) {
            let h = self.handshake.take().unwrap();
            self.remote = h.remote_static();
            self.transport = Some(h.into_transport().map_err(fail)?);
        }
        Ok(())
    }
    #[wasm_bindgen(getter)]
    pub fn ready(&self) -> bool {
        self.transport.is_some()
    }
    #[wasm_bindgen(getter, js_name = remoteKey)]
    pub fn remote_key(&self) -> Option<Vec<u8>> {
        match &self.handshake {
            Some(h) => h.remote_static(),
            None => self.remote.clone(),
        }
    }
    /// False until the first transport message decrypts.
    #[wasm_bindgen(getter)]
    pub fn confirmed(&self) -> bool {
        self.confirmed
    }
    pub fn encrypt(&mut self, plain: &[u8]) -> Result<Vec<u8>, JsError> {
        self.transport.as_mut().ok_or_else(|| JsError::new("not ready"))?.encrypt(plain).map_err(fail)
    }
    pub fn decrypt(&mut self, cipher: &[u8]) -> Result<Vec<u8>, JsError> {
        let out = self.transport.as_mut().ok_or_else(|| JsError::new("not ready"))?.decrypt(cipher).map_err(fail)?;
        self.confirmed = true;
        Ok(out)
    }
}
