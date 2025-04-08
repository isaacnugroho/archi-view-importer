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
