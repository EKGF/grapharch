use {
    std::path::Path,
    tokio::{fs::File, io::AsyncReadExt},
}; // for read_to_end()

/// Asynchronously reads the content of a local file into a string.
pub async fn contents_of_local_file(
    file_path: &Path,
) -> anyhow::Result<String> {
    // Open the file asynchronously
    let mut file = File::open(file_path).await?;

    // Prepare a string buffer
    let mut contents = String::new();

    // Read the entire file into `contents`
    file.read_to_string(&mut contents).await?;

    Ok(contents)
}
