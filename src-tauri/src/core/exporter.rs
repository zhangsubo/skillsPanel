use crate::core::error::AppError;
use crate::core::fs_utils;
use crate::core::models::Skill;
use std::fs;
use std::path::Path;

pub struct Exporter;

impl Exporter {
    pub fn export_to_folder(skill: &Skill, target_dir: &Path) -> Result<(), AppError> {
        let source = Path::new(&skill.library_path);
        if !source.exists() {
            return Err(AppError::SkillNotFound(skill.name.clone()));
        }
        let dest = target_dir.join(&skill.name);
        if dest.exists() {
            fs::remove_dir_all(&dest)?;
        }
        fs_utils::copy_dir_recursive(source, &dest)?;
        Ok(())
    }

    pub fn export_to_zip(skills: &[Skill], target_path: &Path) -> Result<(), AppError> {
        let file = fs::File::create(target_path)?;
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        for skill in skills {
            let source = Path::new(&skill.library_path);
            if !source.exists() {
                continue;
            }
            add_dir_to_zip(&mut zip, source, &skill.name, &options)?;
        }

        zip.finish()?;
        Ok(())
    }
}

fn add_dir_to_zip(
    zip: &mut zip::ZipWriter<fs::File>,
    src: &Path,
    prefix: &str,
    options: &zip::write::SimpleFileOptions,
) -> Result<(), AppError> {
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let name = format!("{}/{}", prefix, entry.file_name().to_string_lossy());
        if entry.file_type()?.is_dir() {
            zip.add_directory(&name, *options)?;
            add_dir_to_zip(zip, &entry.path(), &name, options)?;
        } else {
            zip.start_file(&name, *options)?;
            let content = fs::read(entry.path())?;
            std::io::Write::write_all(zip, &content)?;
        }
    }
    Ok(())
}
