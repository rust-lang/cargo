//! Tests for the `cargo config` command.

use super::config::write_config_at;
use crate::prelude::*;
use cargo_test_support::paths;
use cargo_test_support::str;
use std::fs;
use std::path::PathBuf;

mod help;

fn cargo_process(s: &str) -> cargo_test_support::Execs {
    let mut p = crate::utils::cargo_process(s);
    // Clear out some of the environment added by the default cargo_process so
    // the tests don't need to deal with it.
    p.env_remove("CARGO_PROFILE_DEV_SPLIT_DEBUGINFO")
        .env_remove("CARGO_PROFILE_TEST_SPLIT_DEBUGINFO")
        .env_remove("CARGO_PROFILE_RELEASE_SPLIT_DEBUGINFO")
        .env_remove("CARGO_PROFILE_BENCH_SPLIT_DEBUGINFO")
        .env_remove("CARGO_INCREMENTAL");
    p
}

#[cargo_test]
fn gated() {
    cargo_process("config get")
        .masquerade_as_nightly_cargo(&["cargo-config"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] the `cargo config` command is unstable, pass `-Z unstable-options` to enable it
See https://github.com/rust-lang/cargo/issues/9301 for more information about the `cargo config` command.

"#]])
        .run();
}

fn common_setup() -> PathBuf {
    write_config_at(
        paths::home().join(".cargo/config.toml"),
        "
        [alias]
        foo = \"abc --xyz\"
        [build]
        jobs = 99
        rustflags = [\"--flag-global\"]
        [profile.dev]
        opt-level = 3
        [profile.dev.package.foo]
        opt-level = 1
        [target.'cfg(target_os = \"linux\")']
        runner = \"runme\"

        # How unknown keys are handled.
        [extra-table]
        somekey = \"somevalue\"
        ",
    );
    let sub_folder = paths::root().join("foo/.cargo");
    write_config_at(
        sub_folder.join("config.toml"),
        "
        [alias]
        sub-example = [\"sub\", \"example\"]
        [build]
        rustflags = [\"--flag-directory\"]
        ",
    );
    sub_folder
}

fn array_setup() -> PathBuf {
    let home = paths::home();
    write_config_at(
        home.join(".cargo/config.toml"),
        r#"
        ints = [1, 2, 3]

        bools = [true, false, true]

        strings = ["hello", "world", "test"]

        nested_ints = [[1, 2], [3, 4]]
        nested_bools = [[true], [false, true]]
        nested_strings = [["a", "b"], ["3", "4"]]
        nested_tables = [
            [
                { x = "a" },
                { x = "b" },
            ],
            [
                { x = "c" },
                { x = "d" },
            ],
        ]
        deeply_nested = [[
            { x = [[[ { x = [], y = 2  } ]]], y = 1 },
        ]]

        mixed = [{ x = 1 }, true, [false], "hello", 123]

        [[tables]]
        name = "first"
        value = 1
        [[tables]]
        name = "second"
        value = 2

        "#,
    );
    home
}

#[cargo_test]
fn get_toml() {
    // Notes:
    // - The "extra-table" is shown without a warning. I'm not sure how that
    //   should be handled, since displaying warnings could cause problems
    //   with ingesting the output.
    // - Environment variables aren't loaded. :(
    let sub_folder = common_setup();
    cargo_process("config get -Zunstable-options")
        .cwd(&sub_folder.parent().unwrap())
        .masquerade_as_nightly_cargo(&["cargo-config"])
        .env("CARGO_ALIAS_BAR", "cat dog")
        .env("CARGO_BUILD_JOBS", "100")
        // The weird forward slash in the linux line is due to testsuite normalization.
        .with_stdout_data(str![[r#"
alias.foo = "abc --xyz"
alias.sub-example = ["sub", "example"]
build.jobs = 99
build.rustflags = ["--flag-global", "--flag-directory"]
extra-table.somekey = "somevalue"
profile.dev.opt-level = 3
profile.dev.package.foo.opt-level = 1
target.'cfg(target_os = "linux")'.runner = "runme"
# The following environment variables may affect the loaded values.
# CARGO_ALIAS_BAR=[..]cat dog[..]
# CARGO_BUILD_JOBS=100
# CARGO_HOME=[ROOT]/home/.cargo

"#]])
        .with_stderr_data(str![[r#""#]])
        .run();

    // Env keys work if they are specific.
    cargo_process("config get build.jobs -Zunstable-options")
        .cwd(&sub_folder.parent().unwrap())
        .masquerade_as_nightly_cargo(&["cargo-config"])
        .env("CARGO_BUILD_JOBS", "100")
        .with_stdout_data(str![[r#"
build.jobs = 100

"#]])
        .with_stderr_data(str![[r#""#]])
        .run();

    // Array value.
    cargo_process("config get build.rustflags -Zunstable-options")
        .cwd(&sub_folder.parent().unwrap())
        .masquerade_as_nightly_cargo(&["cargo-config"])
        .with_stdout_data(str![[r#"
build.rustflags = ["--flag-global", "--flag-directory"]

"#]])
        .with_stderr_data(str![[r#""#]])
        .run();

    // Sub-table
    cargo_process("config get profile -Zunstable-options")
        .cwd(&sub_folder.parent().unwrap())
        .masquerade_as_nightly_cargo(&["cargo-config"])
        .with_stdout_data(str![[r#"
profile.dev.opt-level = 3
profile.dev.package.foo.opt-level = 1

"#]])
        .with_stderr_data(str![[r#""#]])
        .run();

    // Specific profile entry.
    cargo_process("config get profile.dev.opt-level -Zunstable-options")
        .cwd(&sub_folder.parent().unwrap())
        .masquerade_as_nightly_cargo(&["cargo-config"])
        .with_stdout_data(str![[r#"
profile.dev.opt-level = 3

"#]])
        .with_stderr_data(str![[r#""#]])
        .run();

    // A key that isn't set.
    cargo_process("config get build.rustc -Zunstable-options")
        .cwd(&sub_folder.parent().unwrap())
        .masquerade_as_nightly_cargo(&["cargo-config"])
        .with_status(101)
        .with_stdout_data(str![[r#""#]])
        .with_stderr_data(str![[r#"
[ERROR] config value `build.rustc` is not set

"#]])
        .run();

    // A key that is not part of Cargo's config schema.
    cargo_process("config get not.set -Zunstable-options")
        .cwd(&sub_folder.parent().unwrap())
        .masquerade_as_nightly_cargo(&["cargo-config"])
        .with_status(101)
        .with_stdout_data(str![[r#""#]])
        .with_stderr_data(str![[r#"
[ERROR] config value `not.set` is not set

"#]])
        .run();
}

#[cargo_test]
fn get_toml_with_array_any_types() {
    let cwd = &array_setup();
    cargo_process("config get -Zunstable-options")
        .cwd(cwd)
        .masquerade_as_nightly_cargo(&["cargo-config"])
        .with_stdout_data(str![[r#"
bools = [true, false, true]
deeply_nested = [[{ x = [[[{ x = [], y = 2 }]]], y = 1 }]]
ints = [1, 2, 3]
mixed = [{ x = 1 }, true, [false], "hello", 123]
nested_bools = [[true], [false, true]]
nested_ints = [[1, 2], [3, 4]]
nested_strings = [["a", "b"], ["3", "4"]]
nested_tables = [[{ x = "a" }, { x = "b" }], [{ x = "c" }, { x = "d" }]]
strings = ["hello", "world", "test"]
tables = [{ name = "first", value = 1 }, { name = "second", value = 2 }]
# The following environment variables may affect the loaded values.
# CARGO_HOME=[ROOT]/home/.cargo

"#]])
        .with_stderr_data(str![[r#""#]])
        .run();

    // Unfortunately there is no TOML syntax to index an array item.
    cargo_process("config get tables -Zunstable-options")
        .cwd(cwd)
        .masquerade_as_nightly_cargo(&["cargo-config"])
        .with_stdout_data(str![[r#"
tables = [{ name = "first", value = 1 }, { name = "second", value = 2 }]

"#]])
        .with_stderr_data(str![[r#""#]])
        .run();
}

#[cargo_test]
fn get_json() {
    let sub_folder = common_setup();
    cargo_process("config get --format=json -Zunstable-options")
        .cwd(&sub_folder.parent().unwrap())
        .masquerade_as_nightly_cargo(&["cargo-config"])
        .env("CARGO_ALIAS_BAR", "cat dog")
        .env("CARGO_BUILD_JOBS", "100")
        .with_stdout_data(
            r#"
{
  "alias": {
    "foo": "abc --xyz",
    "sub-example": [
      "sub",
      "example"
    ]
  },
  "build": {
    "jobs": 99,
    "rustflags": [
      "--flag-global",
      "--flag-directory"
    ]
  },
  "extra-table": {
    "somekey": "somevalue"
  },
  "profile": {
    "dev": {
      "opt-level": 3,
      "package": {
        "foo": {
          "opt-level": 1
        }
      }
    }
  },
  "target": {
    "cfg(target_os = \"linux\")": {
      "runner": "runme"
    }
  }
}

"#
            .is_json(),
        )
        .with_stderr_data(str![[r#"
[NOTE] The following environment variables may affect the loaded values.
CARGO_ALIAS_BAR=[..]cat dog[..]
CARGO_BUILD_JOBS=100
CARGO_HOME=[ROOT]/home/.cargo

"#]])
        .run();

    // json-value is the same for the entire root table
    cargo_process("config get --format=json-value -Zunstable-options")
        .cwd(&sub_folder.parent().unwrap())
        .masquerade_as_nightly_cargo(&["cargo-config"])
        .with_stdout_data(
            r#"
{
  "alias": {
    "foo": "abc --xyz",
    "sub-example": [
      "sub",
      "example"
    ]
  },
  "build": {
    "jobs": 99,
    "rustflags": [
      "--flag-global",
      "--flag-directory"
    ]
  },
  "extra-table": {
    "somekey": "somevalue"
  },
  "profile": {
    "dev": {
      "opt-level": 3,
      "package": {
        "foo": {
          "opt-level": 1
        }
      }
    }
  },
  "target": {
    "cfg(target_os = \"linux\")": {
      "runner": "runme"
    }
  }
}
  
"#
            .is_json(),
        )
        .with_stderr_data(str![[r#"
[NOTE] The following environment variables may affect the loaded values.
CARGO_HOME=[ROOT]/home/.cargo

"#]])
        .run();

    cargo_process("config get --format=json build.jobs -Zunstable-options")
        .cwd(&sub_folder.parent().unwrap())
        .masquerade_as_nightly_cargo(&["cargo-config"])
        .with_stdout_data(str![[r#"
{"build":{"jobs":99}}

"#]])
        .with_stderr_data(str![[r#""#]])
        .run();

    cargo_process("config get --format=json-value build.jobs -Zunstable-options")
        .cwd(&sub_folder.parent().unwrap())
        .masquerade_as_nightly_cargo(&["cargo-config"])
        .with_stdout_data(str![[r#"
99

"#]])
        .with_stderr_data(str![[r#""#]])
        .run();
}

#[cargo_test]
fn get_json_with_array_any_types() {
    let cwd = &array_setup();
    cargo_process("config get --format=json -Zunstable-options")
        .cwd(cwd)
        .masquerade_as_nightly_cargo(&["cargo-config"])
        .with_stdout_data(
            str![[r#"
{
  "bools": [
    true,
    false,
    true
  ],
  "deeply_nested": [
    [
      {
        "x": [
          [
            [
              {
                "x": [],
                "y": 2
              }
            ]
          ]
        ],
        "y": 1
      }
    ]
  ],
  "ints": [
    1,
    2,
    3
  ],
  "mixed": [
    {
      "x": 1
    },
    true,
    [
      false
    ],
    "hello",
    123
  ],
  "nested_bools": [
    [
      true
    ],
    [
      false,
      true
    ]
  ],
  "nested_ints": [
    [
      1,
      2
    ],
    [
      3,
      4
    ]
  ],
  "nested_strings": [
    [
      "a",
      "b"
    ],
    [
      "3",
      "4"
    ]
  ],
  "nested_tables": [
    [
      {
        "x": "a"
      },
      {
        "x": "b"
      }
    ],
    [
      {
        "x": "c"
      },
      {
        "x": "d"
      }
    ]
  ],
  "strings": [
    "hello",
    "world",
    "test"
  ],
  "tables": [
    {
      "name": "first",
      "value": 1
    },
    {
      "name": "second",
      "value": 2
    }
  ]
}
"#]]
            .is_json(),
        )
        .with_stderr_data(str![[r#"
[NOTE] The following environment variables may affect the loaded values.
CARGO_HOME=[ROOT]/home/.cargo

"#]])
        .run();

    // Unfortunately there is no TOML syntax to index an array item.
    cargo_process("config get tables --format=json -Zunstable-options")
        .cwd(cwd)
        .masquerade_as_nightly_cargo(&["cargo-config"])
        .with_stdout_data(
            str![[r#"
{
  "tables": [
    {
      "name": "first",
      "value": 1
    },
    {
      "name": "second",
      "value": 2
    }
  ]
}
"#]]
            .is_json(),
        )
        .with_stderr_data(str![""])
        .run();
}

#[cargo_test]
fn show_origin_toml() {
    let sub_folder = common_setup();
    cargo_process("config get --show-origin -Zunstable-options")
        .cwd(&sub_folder.parent().unwrap())
        .masquerade_as_nightly_cargo(&["cargo-config"])
        .with_stdout_data(str![[r#"
alias.foo = "abc --xyz" # [ROOT]/home/.cargo/config.toml
alias.sub-example = [
    "sub", # [ROOT]/foo/.cargo/config.toml
    "example", # [ROOT]/foo/.cargo/config.toml
]
build.jobs = 99 # [ROOT]/home/.cargo/config.toml
build.rustflags = [
    "--flag-global", # [ROOT]/home/.cargo/config.toml
    "--flag-directory", # [ROOT]/foo/.cargo/config.toml
]
extra-table.somekey = "somevalue" # [ROOT]/home/.cargo/config.toml
profile.dev.opt-level = 3 # [ROOT]/home/.cargo/config.toml
profile.dev.package.foo.opt-level = 1 # [ROOT]/home/.cargo/config.toml
target.'cfg(target_os = "linux")'.runner = "runme" # [ROOT]/home/.cargo/config.toml
# The following environment variables may affect the loaded values.
# CARGO_HOME=[ROOT]/home/.cargo

"#]])
        .with_stderr_data(str![[r#""#]])
        .run();

    cargo_process("config get --show-origin build.rustflags -Zunstable-options")
        .cwd(&sub_folder.parent().unwrap())
        .masquerade_as_nightly_cargo(&["cargo-config"])
        .env("CARGO_BUILD_RUSTFLAGS", "env1 env2")
        .with_stdout_data(str![[r#"
build.rustflags = [
    "--flag-global", # [ROOT]/home/.cargo/config.toml
    "--flag-directory", # [ROOT]/foo/.cargo/config.toml
    "env1", # environment variable `CARGO_BUILD_RUSTFLAGS`
    "env2", # environment variable `CARGO_BUILD_RUSTFLAGS`
]

"#]])
        .with_stderr_data(str![[r#""#]])
        .run();
}

#[cargo_test]
fn show_origin_toml_with_array_any_types() {
    let cwd = &array_setup();
    cargo_process("config get --show-origin -Zunstable-options")
        .cwd(cwd)
        .masquerade_as_nightly_cargo(&["cargo-config"])
        .with_stdout_data(str![[r#"
bools = [
    true, # [ROOT]/home/.cargo/config.toml
    false, # [ROOT]/home/.cargo/config.toml
    true, # [ROOT]/home/.cargo/config.toml
]
deeply_nested = [
    [{ x = [[[{ x = [], y = 2 }]]], y = 1 }], # [ROOT]/home/.cargo/config.toml
]
ints = [
    1, # [ROOT]/home/.cargo/config.toml
    2, # [ROOT]/home/.cargo/config.toml
    3, # [ROOT]/home/.cargo/config.toml
]
mixed = [
    { x = 1 }, # [ROOT]/home/.cargo/config.toml
    true, # [ROOT]/home/.cargo/config.toml
    [false], # [ROOT]/home/.cargo/config.toml
    "hello", # [ROOT]/home/.cargo/config.toml
    123, # [ROOT]/home/.cargo/config.toml
]
nested_bools = [
    [true], # [ROOT]/home/.cargo/config.toml
    [false, true], # [ROOT]/home/.cargo/config.toml
]
nested_ints = [
    [1, 2], # [ROOT]/home/.cargo/config.toml
    [3, 4], # [ROOT]/home/.cargo/config.toml
]
nested_strings = [
    ["a", "b"], # [ROOT]/home/.cargo/config.toml
    ["3", "4"], # [ROOT]/home/.cargo/config.toml
]
nested_tables = [
    [{ x = "a" }, { x = "b" }], # [ROOT]/home/.cargo/config.toml
    [{ x = "c" }, { x = "d" }], # [ROOT]/home/.cargo/config.toml
]
strings = [
    "hello", # [ROOT]/home/.cargo/config.toml
    "world", # [ROOT]/home/.cargo/config.toml
    "test", # [ROOT]/home/.cargo/config.toml
]
tables = [
    { name = "first", value = 1 }, # [ROOT]/home/.cargo/config.toml
    { name = "second", value = 2 }, # [ROOT]/home/.cargo/config.toml
]
# The following environment variables may affect the loaded values.
# CARGO_HOME=[ROOT]/home/.cargo

"#]])
        .with_stderr_data(str![[r#""#]])
        .run();

    // Unfortunately there is no TOML syntax to index an array item.
    cargo_process("config get tables --show-origin -Zunstable-options")
        .cwd(cwd)
        .masquerade_as_nightly_cargo(&["cargo-config"])
        .with_stdout_data(str![[r#"
tables = [
    { name = "first", value = 1 }, # [ROOT]/home/.cargo/config.toml
    { name = "second", value = 2 }, # [ROOT]/home/.cargo/config.toml
]

"#]])
        .with_stderr_data(str![[r#""#]])
        .run();
}

#[cargo_test]
fn show_origin_toml_cli() {
    let sub_folder = common_setup();
    cargo_process("config get --show-origin build.jobs -Zunstable-options --config build.jobs=123")
        .cwd(&sub_folder.parent().unwrap())
        .masquerade_as_nightly_cargo(&["cargo-config"])
        .env("CARGO_BUILD_JOBS", "1")
        .with_stdout_data(str![[r#"
build.jobs = 123 # --config cli option

"#]])
        .with_stderr_data(str![[r#""#]])
        .run();

    cargo_process("config get --show-origin build.rustflags -Zunstable-options --config")
        .arg("build.rustflags=[\"cli1\",\"cli2\"]")
        .cwd(&sub_folder.parent().unwrap())
        .masquerade_as_nightly_cargo(&["cargo-config"])
        .env("CARGO_BUILD_RUSTFLAGS", "env1 env2")
        .with_stdout_data(str![[r#"
build.rustflags = [
    "--flag-global", # [ROOT]/home/.cargo/config.toml
    "--flag-directory", # [ROOT]/foo/.cargo/config.toml
    "env1", # environment variable `CARGO_BUILD_RUSTFLAGS`
    "env2", # environment variable `CARGO_BUILD_RUSTFLAGS`
    "cli1", # --config cli option
    "cli2", # --config cli option
]

"#]])
        .with_stderr_data(str![[r#""#]])
        .run();
}

#[cargo_test]
fn show_origin_json() {
    let sub_folder = common_setup();
    cargo_process("config get --show-origin --format=json -Zunstable-options")
        .cwd(&sub_folder.parent().unwrap())
        .masquerade_as_nightly_cargo(&["cargo-config"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] the `json` format does not support --show-origin, try the `toml` format instead

"#]])
        .run();
}

#[cargo_test]
fn unmerged_toml() {
    let sub_folder = common_setup();
    cargo_process("config get --merged=no -Zunstable-options")
        .cwd(&sub_folder.parent().unwrap())
        .masquerade_as_nightly_cargo(&["cargo-config"])
        .env("CARGO_ALIAS_BAR", "cat dog")
        .env("CARGO_BUILD_JOBS", "100")
        .with_stdout_data(str![[r##"
# Environment variables
# CARGO=[..]
# CARGO_ALIAS_BAR=[..]cat dog[..]
# CARGO_BUILD_JOBS=100
# CARGO_HOME=[ROOT]/home/.cargo

# [ROOT]/foo/.cargo/config.toml
alias.sub-example = ["sub", "example"]
build.rustflags = ["--flag-directory"]

# [ROOT]/home/.cargo/config.toml
alias.foo = "abc --xyz"
build.jobs = 99
build.rustflags = ["--flag-global"]
extra-table.somekey = "somevalue"
profile.dev.opt-level = 3
profile.dev.package.foo.opt-level = 1
target.'cfg(target_os = "linux")'.runner = "runme"


"##]])
        .with_stderr_data(str![[r#""#]])
        .run();

    cargo_process("config get --merged=no build.rustflags -Zunstable-options")
        .cwd(&sub_folder.parent().unwrap())
        .masquerade_as_nightly_cargo(&["cargo-config"])
        .env("CARGO_BUILD_RUSTFLAGS", "env1 env2")
        .with_stdout_data(str![[r#"
# Environment variables
# CARGO_BUILD_RUSTFLAGS=[..]env1 env2[..]

# [ROOT]/foo/.cargo/config.toml
build.rustflags = ["--flag-directory"]

# [ROOT]/home/.cargo/config.toml
build.rustflags = ["--flag-global"]


"#]])
        .with_stderr_data(str![[r#""#]])
        .run();

    cargo_process("config get --merged=no does.not.exist -Zunstable-options")
        .cwd(&sub_folder.parent().unwrap())
        .masquerade_as_nightly_cargo(&["cargo-config"])
        .with_stderr_data(str![[r#""#]])
        .with_stderr_data(str![[r#""#]])
        .run();

    cargo_process("config get --merged=no build.rustflags.extra -Zunstable-options")
        .cwd(&sub_folder.parent().unwrap())
        .masquerade_as_nightly_cargo(&["cargo-config"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] expected table for configuration key `build.rustflags`, but found array in [ROOT]/foo/.cargo/config.toml

"#]])
        .run();
}

#[cargo_test]
fn unmerged_toml_cli() {
    let sub_folder = common_setup();
    cargo_process("config get --merged=no build.rustflags -Zunstable-options --config")
        .arg("build.rustflags=[\"cli1\",\"cli2\"]")
        .cwd(&sub_folder.parent().unwrap())
        .masquerade_as_nightly_cargo(&["cargo-config"])
        .env("CARGO_BUILD_RUSTFLAGS", "env1 env2")
        .with_stdout_data(str![[r#"
# --config cli option
build.rustflags = ["cli1", "cli2"]

# Environment variables
# CARGO_BUILD_RUSTFLAGS=[..]env1 env2[..]

# [ROOT]/foo/.cargo/config.toml
build.rustflags = ["--flag-directory"]

# [ROOT]/home/.cargo/config.toml
build.rustflags = ["--flag-global"]


"#]])
        .with_stderr_data(str![[r#""#]])
        .run();
}

#[cargo_test]
fn unmerged_json() {
    let sub_folder = common_setup();
    cargo_process("config get --merged=no --format=json -Zunstable-options")
        .cwd(&sub_folder.parent().unwrap())
        .masquerade_as_nightly_cargo(&["cargo-config"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] the `json` format does not support --merged=no, try the `toml` format instead

"#]])
        .run();
}

#[cargo_test]
fn includes() {
    let sub_folder = common_setup();
    fs::write(
        sub_folder.join("config.toml"),
        "
        include = 'other.toml'
        [build]
        rustflags = [\"--flag-directory\"]
        ",
    )
    .unwrap();
    fs::write(
        sub_folder.join("other.toml"),
        "
        [build]
        rustflags = [\"--flag-other\"]
        ",
    )
    .unwrap();

    cargo_process("config get build.rustflags -Zunstable-options -Zconfig-include")
        .cwd(&sub_folder.parent().unwrap())
        .masquerade_as_nightly_cargo(&["cargo-config", "config-include"])
        .with_stdout_data(str![[r#"
build.rustflags = ["--flag-global", "--flag-other", "--flag-directory"]

"#]])
        .with_stderr_data(str![[r#""#]])
        .run();

    cargo_process("config get build.rustflags --show-origin -Zunstable-options -Zconfig-include")
        .cwd(&sub_folder.parent().unwrap())
        .masquerade_as_nightly_cargo(&["cargo-config", "config-include"])
        .with_stdout_data(str![[r#"
build.rustflags = [
    "--flag-global", # [ROOT]/home/.cargo/config.toml
    "--flag-other", # [ROOT]/foo/.cargo/other.toml
    "--flag-directory", # [ROOT]/foo/.cargo/config.toml
]

"#]])
        .with_stderr_data(str![[r#""#]])
        .run();

    cargo_process("config get --merged=no -Zunstable-options -Zconfig-include")
        .cwd(&sub_folder.parent().unwrap())
        .masquerade_as_nightly_cargo(&["cargo-config", "config-include"])
        .with_stdout_data(str![[r##"
# Environment variables
# CARGO=[..]
# CARGO_HOME=[ROOT]/home/.cargo

# [ROOT]/foo/.cargo/other.toml
build.rustflags = ["--flag-other"]

# [ROOT]/foo/.cargo/config.toml
build.rustflags = ["--flag-directory"]
include = "other.toml"

# [ROOT]/home/.cargo/config.toml
alias.foo = "abc --xyz"
build.jobs = 99
build.rustflags = ["--flag-global"]
extra-table.somekey = "somevalue"
profile.dev.opt-level = 3
profile.dev.package.foo.opt-level = 1
target.'cfg(target_os = "linux")'.runner = "runme"


"##]])
        .with_stderr_data(str![[r#""#]])
        .run();
}
