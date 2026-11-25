//! The installer art as raw PNG bytes; decoding comes with the window. The files stay
//! single-copy in `docs/design/identity/r3/installer/`.

/// One art file from the design folder.
macro_rules! art {
    ($name:literal) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/design/identity/r3/installer/",
            $name
        ))
    };
}

/// The background of each page as (1x, 2x) PNG bytes.
pub const BACKGROUNDS: [(&[u8], &[u8]); 4] = [
    (art!("bg-0-1x.png"), art!("bg-0-2x.png")),
    (art!("bg-1-1x.png"), art!("bg-1-2x.png")),
    (art!("bg-2-1x.png"), art!("bg-2-2x.png")),
    (art!("bg-3-1x.png"), art!("bg-3-2x.png")),
];

/// The button as (1x, 2x) PNG bytes.
pub const BUTTON: (&[u8], &[u8]) = (art!("button-1x.png"), art!("button-2x.png"));
