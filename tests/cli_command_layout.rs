#[test]
fn publication_command_keeps_backend_options_indirect() {
    // Windows PathBuf is larger than Unix PathBuf. Keep CLI alternatives
    // bounded on both platforms instead of suppressing large_enum_variant.
    assert!(
        std::mem::size_of::<grapharch::publication::cli::PublicationCommand>() <=
            336,
        "publication backend configuration must not inflate every command"
    );
}
