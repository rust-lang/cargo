use std::path::Path;

pub(crate) fn description(path: &Path) -> Option<String> {
    implementation::description(path)
}

#[cfg(target_os = "linux")]
mod implementation {
    use memmap::Mmap;
    use object::endian::LittleEndian;
    use object::read::elf::{ElfFile64, FileHeader, SectionHeader};
    use std::fs::File;
    use std::path::Path;
    use std::str;

    pub(super) fn description(path: &Path) -> Option<String> {
        let executable_file = File::open(path).ok()?;
        let data = &*unsafe { Mmap::map(&executable_file) }.ok()?;
        let elf = ElfFile64::<LittleEndian>::parse(data).ok()?;
        let endian = elf.endian();
        let file_header = elf.raw_header();
        let section_headers = file_header.section_headers(endian, data).ok()?;
        let string_table = file_header
            .section_strings(endian, data, section_headers)
            .ok()?;

        let mut description = None;
        for section_header in section_headers {
            if section_header.name(endian, string_table).ok() == Some(b".note.cargo.subcommand") {
                if let Ok(Some(mut notes)) = section_header.notes(endian, data) {
                    while let Ok(Some(note)) = notes.next() {
                        if note.name() == cargo_subcommand_metadata::ELF_NOTE_NAME.as_bytes()
                            && note.n_type(endian)
                                == cargo_subcommand_metadata::ElfNoteType::Description as u32
                        {
                            if description.is_some() {
                                return None;
                            }
                            description = Some(note.desc());
                        }
                    }
                }
            }
        }

        let description: &[u8] = description?;
        let description: &str = str::from_utf8(description).ok()?;
        if description.len() > 280 || description.contains(&['\n', '\r', '\x1B']) {
            return None;
        }

        Some(description.to_owned())
    }
}

#[cfg(not(target_os = "linux"))]
mod implementation {
    use std::path::Path;

    pub(super) fn description(_path: &Path) -> Option<String> {
        None
    }
}
