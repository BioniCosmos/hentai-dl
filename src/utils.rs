pub(crate) fn media_type_to_ext(media_type: &str) -> Option<&str> {
    const M: &[(&str, &str)] = &[
        ("image/avif", "avif"),
        ("image/bmp", "bmp"),
        ("image/heic", "heic"),
        ("image/jpeg", "jpeg"),
        ("image/png", "png"),
        ("image/webp", "webp"),
    ];
    M.iter()
        .find(|(t, _)| media_type.contains(t))
        .map(|(_, ext)| *ext)
}
