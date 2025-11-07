pub trait Hex {
    fn to_hex(&self) -> String;
}

impl<T: core::borrow::Borrow<[u8]>> Hex for T {
    fn to_hex(&self) -> String {
        hex::encode(self.borrow())
    }
}
