use encoding_rs::UTF_8;
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use zip::write::FileOptions;
use zip::CompressionMethod;
use zip::{ZipArchive, ZipWriter};

#[derive(Debug)]
pub enum FileDescriptor {
    PlainXml {
        path: PathBuf,
    },
    ZippedXml {
        zip_path: PathBuf,
        xml_filename: String,
    },
}

impl FileDescriptor {
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let path = path.as_ref().to_path_buf();

        // Try as plain XML
        if let Ok(bytes) = fs::read(&path) {
            let (decoded, _had_errors) = UTF_8.decode_without_bom_handling(&bytes);
            if decoded.contains("<?xml") {
                return Ok(FileDescriptor::PlainXml { path });
            }
        }

        // Try as zip file containing an XML
        if let Ok(file) = fs::File::open(&path) {
            let mut archive = ZipArchive::new(file)?;
            for i in 0..archive.len() {
                let file = archive.by_index(i)?;
                let name = file.name();

                if name.eq("model.xml") {
                    return Ok(FileDescriptor::ZippedXml {
                        zip_path: path,
                        xml_filename: name.to_string(),
                    });
                }
            }
        }

        Err("Could not determine file type or locate XML".into())
    }

    pub fn read_xml(&self) -> Result<String, Box<dyn std::error::Error>> {
        match self {
            FileDescriptor::PlainXml { path, .. } => {
                let bytes = fs::read(path)?;
                let (decoded, _, _) = UTF_8.decode(&bytes);
                Ok(decoded.into())
            }
            FileDescriptor::ZippedXml {
                zip_path,
                xml_filename,
                ..
            } => {
                let file = fs::File::open(zip_path)?;
                let mut archive = ZipArchive::new(file)?;

                let mut xml_file = archive.by_name(xml_filename)?;
                let mut buffer = Vec::new();
                xml_file.read_to_end(&mut buffer)?;

                let (decoded, _, _) = UTF_8.decode(&buffer);
                Ok(decoded.into())
            }
        }
    }

    pub fn write_xml(&self, new_xml: &str) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            FileDescriptor::PlainXml { path, .. } => {
                fs::write(path, new_xml.as_bytes())?;
            }
            FileDescriptor::ZippedXml {
                zip_path,
                xml_filename,
                ..
            } => {
                let zip_data = fs::read(zip_path)?;
                let reader = Cursor::new(zip_data);
                let mut archive = ZipArchive::new(reader)?;

                let mut buffer = Cursor::new(Vec::new());
                let mut zip_writer = ZipWriter::new(&mut buffer);

                for i in 0..archive.len() {
                    let mut file = archive.by_index(i)?;
                    let name = file.name().to_string();

                    let options: FileOptions<()> =
                        FileOptions::default().compression_method(CompressionMethod::Stored);

                    zip_writer.start_file(name.clone(), options)?;

                    if name == *xml_filename {
                        zip_writer.write_all(new_xml.as_bytes())?;
                    } else {
                        let mut content = Vec::new();
                        file.read_to_end(&mut content)?;
                        zip_writer.write_all(&content)?;
                    }
                }

                zip_writer.finish()?;
                fs::write(zip_path, buffer.into_inner())?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_from_path_plain_xml() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempdir()?;
        let file_path = dir.path().join("test.xml");
        fs::write(&file_path, "<?xml version=\"1.0\"?><root></root>")?;

        let descriptor = FileDescriptor::from_path(&file_path)?;
        match descriptor {
            FileDescriptor::PlainXml { path } => {
                assert_eq!(path, file_path);
                Ok(())
            }
            _ => Err("Expected PlainXml variant".into()),
        }
    }

    #[test]
    fn test_from_path_zipped_xml() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempdir()?;
        let zip_path = dir.path().join("test.zip");
        
        {
            let file = fs::File::create(&zip_path)?;
            let mut zip = ZipWriter::new(file);
            zip.start_file::<_, ()>("model.xml", FileOptions::default())?;
            zip.write_all(b"<?xml version=\"1.0\"?><root></root>")?;
            zip.finish()?;
        }

        let descriptor = FileDescriptor::from_path(&zip_path)?;
        match descriptor {
            FileDescriptor::ZippedXml { zip_path: path, xml_filename } => {
                assert_eq!(path, zip_path);
                assert_eq!(xml_filename, "model.xml");
                Ok(())
            }
            _ => Err("Expected ZippedXml variant".into()),
        }
    }

    #[test]
    fn test_read_write_plain_xml() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempdir()?;
        let file_path = dir.path().join("test.xml");
        let initial_content = "<?xml version=\"1.0\"?><root></root>";
        fs::write(&file_path, initial_content)?;

        let descriptor = FileDescriptor::from_path(&file_path)?;
        assert_eq!(descriptor.read_xml()?, initial_content);

        let new_content = "<?xml version=\"1.0\"?><root><child/></root>";
        descriptor.write_xml(new_content)?;
        assert_eq!(descriptor.read_xml()?, new_content);

        Ok(())
    }

    #[test]
    fn test_read_write_zipped_xml() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempdir()?;
        let zip_path = dir.path().join("test.zip");
        let initial_content = "<?xml version=\"1.0\"?><root></root>";
        
        {
            let file = fs::File::create(&zip_path)?;
            let mut zip = ZipWriter::new(file);
            zip.start_file::<_, ()>("model.xml", FileOptions::default())?;
            zip.write_all(initial_content.as_bytes())?;
            zip.finish()?;
        }

        let descriptor = FileDescriptor::from_path(&zip_path)?;
        assert_eq!(descriptor.read_xml()?, initial_content);

        let new_content = "<?xml version=\"1.0\"?><root><child/></root>";
        descriptor.write_xml(new_content)?;
        assert_eq!(descriptor.read_xml()?, new_content);

        Ok(())
    }

    #[test]
    fn test_invalid_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "not an xml file").unwrap();

        assert!(FileDescriptor::from_path(&file_path).is_err());
    }
}
