use model::color::{Base16, Palette, Rgb, RgbError};

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

#[test]
fn parses_stylix_palette() {
    let palette: Palette =
        serde_json::from_str(include_str!("../fixtures/palette.json")).expect("valid palette");
    assert_eq!(
        palette[Base16::Base00],
        Rgb {
            r: 0x06,
            g: 0x12,
            b: 0x29
        }
    );
    assert_eq!(
        palette[Base16::Base0F],
        Rgb {
            r: 0xb0,
            g: 0x80,
            b: 0x60
        }
    );
}

#[test]
fn palette_missing_slot_fails() {
    let result = serde_json::from_str::<Palette>(r#"{ "base00": "061229" }"#);
    assert!(result.is_err());
}
