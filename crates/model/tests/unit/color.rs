use model::color::{Rgb, RgbError};

#[test]
fn parses_hex_with_and_without_hash() {
    let expected = Rgb {
        r: 0x00,
        g: 0x40,
        b: 0x40,
    };
    assert_eq!("#004040".parse::<Rgb>(), Ok(expected));
    assert_eq!("004040".parse::<Rgb>(), Ok(expected));
}

#[test]
fn rejects_bad_hex() {
    assert!(matches!("#0040".parse::<Rgb>(), Err(RgbError::Length(_))));
    assert!(matches!("#00404g".parse::<Rgb>(), Err(RgbError::Digit(_))));
}
