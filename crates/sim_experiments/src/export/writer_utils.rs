use std::fs::File;
use std::path::Path;

pub(crate) fn ensure_not_empty<T>(items: &[T]) -> Result<(), Box<dyn std::error::Error>> {
    if items.is_empty() {
        return Err("No results to export".into());
    }

    Ok(())
}

pub(crate) fn create_output_file(
    path: impl AsRef<Path>,
) -> Result<File, Box<dyn std::error::Error>> {
    Ok(File::create(path)?)
}
