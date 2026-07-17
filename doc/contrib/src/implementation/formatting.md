# Formatting

When modifying user files, like `Cargo.toml`, we should not change other
sections of the file,
preserving the general formatting.
This includes the table, inline-table, or array that a field is being edited in.

When adding new entries, they do not need to match the canonical style of the
document but can use the default formatting.
If the entry is already sorted, preserving the sort order is preferred.

When removing entries,
comments on the same line should be removed but comments on following lines
should be preserved.

Inconsistencies in style after making a change are left to the user and their
preferred auto-formatter.
