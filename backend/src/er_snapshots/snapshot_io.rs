//! Snapshot file writing and replacement helpers for ER snapshot generation.

use super::{
    MERMAID_FILENAME, MermaidRenderer, SVG_FILENAME, SnapshotArtifacts, SnapshotGenerationError,
    SnapshotRequest,
};
use cap_std::{ambient_authority, fs::Dir};
use std::io;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub(super) fn write_snapshots_atomically(
    mermaid: &str,
    renderer: &dyn MermaidRenderer,
    request: &SnapshotRequest,
) -> Result<SnapshotArtifacts, SnapshotGenerationError> {
    Dir::create_ambient_dir_all(&request.output_dir, ambient_authority())
        .map_err(|error| SnapshotGenerationError::io(&request.output_dir, error))?;
    let output_dir = Dir::open_ambient_dir(&request.output_dir, ambient_authority())
        .map_err(|error| SnapshotGenerationError::io(&request.output_dir, error))?;

    let staging_dir_name = format!(".tmp-er-snapshot-{}", Uuid::new_v4().simple());
    output_dir.create_dir(&staging_dir_name).map_err(|error| {
        SnapshotGenerationError::io(request.output_dir.join(&staging_dir_name), error)
    })?;

    let staged_mermaid_relative = PathBuf::from(&staging_dir_name).join(MERMAID_FILENAME);
    let staged_svg_relative = PathBuf::from(&staging_dir_name).join(SVG_FILENAME);
    let staged_mermaid = request.output_dir.join(&staged_mermaid_relative);
    let staged_svg = request.output_dir.join(&staged_svg_relative);
    let final_mermaid = request.output_dir.join(MERMAID_FILENAME);
    let final_svg = request.output_dir.join(SVG_FILENAME);

    let result = (|| -> Result<SnapshotArtifacts, SnapshotGenerationError> {
        output_dir
            .write(&staged_mermaid_relative, mermaid.as_bytes())
            .map_err(|error| SnapshotGenerationError::io(&staged_mermaid, error))?;

        let svg_path = if request.should_render_svg {
            renderer.render_svg(&staged_mermaid, &staged_svg)?;
            Some(final_svg.clone())
        } else {
            None
        };

        replace_file(
            &output_dir,
            staged_mermaid_relative.as_path(),
            Path::new(MERMAID_FILENAME),
            &request.output_dir,
        )?;
        if request.should_render_svg {
            replace_file(
                &output_dir,
                staged_svg_relative.as_path(),
                Path::new(SVG_FILENAME),
                &request.output_dir,
            )?;
        } else {
            remove_file_if_exists(&output_dir, Path::new(SVG_FILENAME), &request.output_dir)?;
        }

        Ok(SnapshotArtifacts {
            mermaid_path: final_mermaid,
            svg_path,
        })
    })();

    let _cleanup_result = output_dir.remove_dir_all(&staging_dir_name);
    result
}

fn replace_file(
    directory: &Dir,
    from: &Path,
    to: &Path,
    output_dir: &Path,
) -> Result<(), SnapshotGenerationError> {
    match directory.remove_file(to) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(SnapshotGenerationError::io(output_dir.join(to), error));
        }
    }
    directory
        .rename(from, directory, to)
        .map_err(|error| SnapshotGenerationError::io(output_dir.join(to), error))
}

fn remove_file_if_exists(
    directory: &Dir,
    path: &Path,
    output_dir: &Path,
) -> Result<(), SnapshotGenerationError> {
    match directory.remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(SnapshotGenerationError::io(output_dir.join(path), error)),
    }
}
