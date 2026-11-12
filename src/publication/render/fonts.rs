//! Openly licensed fonts embedded in every PDF compiler; host fonts are never
//! used. See `fonts/README.md` for sources, hashes and licenses.

/// bounded: twelve fixed font files.
pub(super) const EMBEDDED: [&[u8]; 12] = [
    include_bytes!("fonts/LiberationSans-Regular.ttf"),
    include_bytes!("fonts/LiberationSans-Bold.ttf"),
    include_bytes!("fonts/LiberationSans-Italic.ttf"),
    include_bytes!("fonts/LiberationSans-BoldItalic.ttf"),
    include_bytes!("fonts/LiberationSerif-Regular.ttf"),
    include_bytes!("fonts/LiberationSerif-Bold.ttf"),
    include_bytes!("fonts/LiberationSerif-Italic.ttf"),
    include_bytes!("fonts/LiberationSerif-BoldItalic.ttf"),
    include_bytes!("fonts/LiberationMono-Regular.ttf"),
    include_bytes!("fonts/LiberationMono-Bold.ttf"),
    include_bytes!("fonts/LiberationMono-Italic.ttf"),
    include_bytes!("fonts/LiberationMono-BoldItalic.ttf"),
];
