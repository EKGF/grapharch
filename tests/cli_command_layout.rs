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

#[test]
fn revision_build_future_is_bounded() {
    let request = grapharch::publication::revision_build::BuildRequest {
        git_repository:     String::new(),
        git_commit:         String::new(),
        definitions:        std::path::PathBuf::new(),
        definitions_commit: String::new(),
        project:            String::new(),
        targets:            Vec::new(),
        all_targets:        false,
        generator_revision: String::new(),
        publication_url:
            grapharch::publication::revision_build::PublicationUrl::try_new(
                "https://example.org/",
            )
            .unwrap(),
        git_cache:          std::path::PathBuf::new(),
        output_root:        std::path::PathBuf::new(),
    };
    let size = std::mem::size_of_val(&request.build());
    assert!(
        size <= 16 * 1024,
        "revision build future occupies {size} bytes"
    );
}
