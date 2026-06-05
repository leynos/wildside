use pagination::Cursor;

struct NotSerialisable {
    value: u32,
}

fn main() {
    let cursor = Cursor::new(NotSerialisable { value: 42 });
    // encode() requires Key: Serialize — must be rejected by rustc
    let _ = cursor.encode();
}
