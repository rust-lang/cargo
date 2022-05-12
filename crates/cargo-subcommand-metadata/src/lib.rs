/// Cargo's name for the purpose of ELF notes.
///
/// The `name` field of an ELF note is designated to hold the entry's "owner" or
/// "originator". No formal mechanism exists for avoiding name conflicts. By
/// convention, vendors use their own name such as "XYZ Computer Company".
pub const ELF_NOTE_NAME: &str = "rust-lang/cargo";

/// Values used by Cargo as the `type` of its ELF notes.
///
/// Each originator controls its own note types. Multiple interpretations of a
/// single type value can exist. A program must recognize both the `name` and
/// the `type` to understand a descriptor.
#[repr(i32)]
#[non_exhaustive]
pub enum ElfNoteType {
    //              DESCRIP
    Description = 0xDE5C819,
}

/// Embed a description into a compiled Cargo subcommand, to be shown by `cargo
/// --list`.
///
/// The following restrictions apply to a subcommand description:
///
/// - String length can be at most 280 bytes in UTF-8, although much shorter is
///   better.
/// - Must not contain the characters `\n`, `\r`, or `\x1B` (ESC).
///
/// Please consider running `cargo --list` and following the style of the
/// existing descriptions of the built-in Cargo subcommands.
///
/// # Example
///
/// ```
/// // subcommand's main.rs
///
/// cargo_subcommand_metadata::description! {
///     "Draw a spiffy visualization of things"
/// }
///
/// fn main() {
///     /* … */
/// }
/// ```
#[macro_export]
macro_rules! description {
    ($description:expr) => {
        const _: () = {
            const CARGO_SUBCOMMAND_DESCRIPTION: &str = $description;

            assert!(
                CARGO_SUBCOMMAND_DESCRIPTION.len() <= 280,
                "subcommand description too long, must be at most 280",
            );

            #[cfg(target_os = "linux")]
            const _: () = {
                #[repr(C)]
                struct ElfNote {
                    namesz: u32,
                    descsz: u32,
                    ty: $crate::ElfNoteType,

                    name: [u8; $crate::ELF_NOTE_NAME.len()],
                    // At least 1 to nul-terminate the string as is convention
                    // (though not required), plus zero padding to a multiple of 4
                    // bytes.
                    name_padding: [$crate::private::Padding;
                        1 + match ($crate::ELF_NOTE_NAME.len() + 1) % 4 {
                            0 => 0,
                            r => 4 - r,
                        }],

                    desc: [u8; CARGO_SUBCOMMAND_DESCRIPTION.len()],
                    // Zero padding to a multiple of 4 bytes.
                    desc_padding: [$crate::private::Padding;
                        match CARGO_SUBCOMMAND_DESCRIPTION.len() % 4 {
                            0 => 0,
                            r => 4 - r,
                        }],
                }

                #[used]
                #[link_section = ".note.cargo.subcommand"]
                static ELF_NOTE: ElfNote = {
                    ElfNote {
                        namesz: $crate::ELF_NOTE_NAME.len() as u32 + 1,
                        descsz: CARGO_SUBCOMMAND_DESCRIPTION.len() as u32,
                        ty: $crate::ElfNoteType::Description,
                        name: unsafe { *$crate::ELF_NOTE_NAME.as_ptr().cast() },
                        name_padding: $crate::private::padding(),
                        desc: unsafe { *CARGO_SUBCOMMAND_DESCRIPTION.as_ptr().cast() },
                        desc_padding: $crate::private::padding(),
                    }
                };
            };
        };
    };
}

// Implementation details. Not public API.
#[doc(hidden)]
pub mod private {
    #[derive(Copy, Clone)]
    #[repr(u8)]
    pub enum Padding {
        Zero = 0,
    }

    pub const fn padding<const N: usize>() -> [Padding; N] {
        [Padding::Zero; N]
    }
}
