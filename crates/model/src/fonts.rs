/// Fonts shipped in the binary, so the defaults work without any installed.
pub const BUNDLED: [&[u8]; 8] = [
    include_bytes!("../../../assets/fonts/lilex/Lilex-Regular.ttf"),
    include_bytes!("../../../assets/fonts/lilex/Lilex-Italic.ttf"),
    include_bytes!("../../../assets/fonts/lilex/Lilex-Bold.ttf"),
    include_bytes!("../../../assets/fonts/lilex/Lilex-BoldItalic.ttf"),
    include_bytes!("../../../assets/fonts/ibm-plex-sans/IBMPlexSans-Regular.ttf"),
    include_bytes!("../../../assets/fonts/ibm-plex-sans/IBMPlexSans-Italic.ttf"),
    include_bytes!("../../../assets/fonts/ibm-plex-sans/IBMPlexSans-SemiBold.ttf"),
    include_bytes!("../../../assets/fonts/ibm-plex-sans/IBMPlexSans-SemiBoldItalic.ttf"),
];

pub const MONO: &str = "Lilex";
pub const SANS: &str = "IBM Plex Sans";
