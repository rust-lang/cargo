//! Utilities to help with working with sqlite.

use crate::CargoResult;
use crate::util::interning::InternedString;
use rusqlite::types::{FromSql, FromSqlError, ToSql, ToSqlOutput};
use rusqlite::{Connection, TransactionBehavior};

impl FromSql for InternedString {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> Result<Self, FromSqlError> {
        value.as_str().map(InternedString::from)
    }
}

impl ToSql for InternedString {
    fn to_sql(&self) -> Result<ToSqlOutput<'_>, rusqlite::Error> {
        Ok(ToSqlOutput::from(self.as_str()))
    }
}

/// A function or closure representing a database migration.
///
/// Migrations support evolving the schema and contents of the database across
/// new versions of cargo. The [`migrate`] function should be called
/// immediately after opening a connection to a database in order to configure
/// the schema. Whether or not a migration has been done is tracked by the
/// `pragma_user_version` value in the database. Typically you include the
/// initial `CREATE TABLE` statements in the initial list, but as time goes on
/// you can add new tables or `ALTER TABLE` statements. The migration code
/// will only execute statements that haven't previously been run.
///
/// Important things to note about how you define migrations:
///
/// * Never remove a migration entry from the list. Migrations are tracked by
///   the index number in the list.
/// * Never perform any schema modifications that would be backwards
///   incompatible. For example, don't drop tables or columns.
///
/// The [`basic_migration`] function is a convenience function for specifying
/// migrations that are simple SQL statements. If you need to do something
/// more complex, then you can specify a closure that takes a [`Connection`]
/// and does whatever is needed.
///
/// For example:
///
/// ```rust
/// # use cargo::util::sqlite::*;
/// # use rusqlite::Connection;
/// # let mut conn = Connection::open_in_memory()?;
/// # fn generate_name() -> String { "example".to_string() };
/// migrate(
///     &mut conn,
///     &[
///         basic_migration(
///             "CREATE TABLE foo (
///                 id INTEGER PRIMARY KEY AUTOINCREMENT,
///                 name STRING NOT NULL
///             )",
///         ),
///         Box::new(|conn| {
///             conn.execute("INSERT INTO foo (name) VALUES (?1)", [generate_name()])?;
///             Ok(())
///         }),
///         basic_migration("ALTER TABLE foo ADD COLUMN size INTEGER"),
///     ],
/// )?;
/// # Ok::<(), anyhow::Error>(())
/// ```
pub type Migration = Box<dyn Fn(&Connection) -> CargoResult<()>>;

/// A basic migration that is a single static SQL statement.
///
/// See [`Migration`] for more information.
pub fn basic_migration(stmt: &'static str) -> Migration {
    Box::new(|conn| {
        conn.execute(stmt, [])?;
        Ok(())
    })
}

/// Perform one-time SQL migrations.
///
/// See [`Migration`] for more information.
pub fn migrate(conn: &mut Connection, migrations: &[Migration]) -> CargoResult<()> {
    // EXCLUSIVE ensures that it starts with an exclusive write lock. No other
    // readers will be allowed. This generally shouldn't be needed if there is
    // a file lock, but might be helpful in cases where cargo's `FileLock`
    // failed.
    let tx = conn.transaction_with_behavior(TransactionBehavior::Exclusive)?;
    let user_version = tx.query_row("SELECT user_version FROM pragma_user_version", [], |row| {
        row.get(0)
    })?;
    if user_version < migrations.len() {
        for migration in &migrations[user_version..] {
            migration(&tx)?;
        }
        tx.pragma_update(None, "user_version", &migrations.len())?;
    }
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrate_twice() -> CargoResult<()> {
        // Check that a second migration will apply.
        let mut conn = Connection::open_in_memory()?;
        let mut migrations = vec![basic_migration("CREATE TABLE foo (a, b, c)")];
        migrate(&mut conn, &migrations)?;
        conn.execute("INSERT INTO foo VALUES (1,2,3)", [])?;
        migrations.push(basic_migration("ALTER TABLE foo ADD COLUMN d"));
        migrate(&mut conn, &migrations)?;
        conn.execute("INSERT INTO foo VALUES (1,2,3,4)", [])?;
        Ok(())
    }
}
