use pagination::Cursor;

struct NotDeserialisable {
    value: u32,
}

fn main() {
    // decode() requires Key: DeserializeOwned — must be rejected by rustc
    let _ = Cursor::<NotDeserialisable>::decode("sometoken");
}
