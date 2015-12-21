use std::fs::File;
use std::io::prelude::*;

use hamcrest::{assert_that, existing_file, is, equal_to};
use rustc_serialize::json::Json;
use support::registry::Package;
use support::{project, execs, basic_bin_manifest};


fn setup() {
}

test!(cargo_metadata_simple {
    let p = project("foo")
            .file("Cargo.toml", &basic_bin_manifest("foo"));

    assert_that(p.cargo_process("metadata"), execs().with_stdout(r#"
[[packages]]
dependencies = []
id = "foo 0.5.0 [..]"
manifest_path = "[..]Cargo.toml"
name = "foo"
version = "0.5.0"

[packages.features]

[[packages.targets]]
kind = ["bin"]
name = "foo"
src_path = "src[..]foo.rs"

[resolve]
package = []

[resolve.root]
dependencies = []
name = "foo"
version = "0.5.0"

"#));
});


test!(cargo_metadata_simple_json {
    let p = project("foo")
            .file("Cargo.toml", &basic_bin_manifest("foo"));

    assert_that(p.cargo_process("metadata").arg("-f").arg("json"), execs().with_stdout(r#"
        {
            "packages": [
                {
                    "name": "foo",
                    "version": "0.5.0",
                    "id": "foo[..]",
                    "source": null,
                    "dependencies": [],
                    "targets": [
                        {
                            "kind": [
                                "bin"
                            ],
                            "name": "foo",
                            "src_path": "src[..]foo.rs"
                        }
                    ],
                    "features": {},
                    "manifest_path": "[..]Cargo.toml"
                }
            ],
            "resolve": {
                "package": [],
                "root": {
                    "name": "foo",
                    "version": "0.5.0",
                    "source": null,
                    "dependencies" : []
                },
                "metadata": null
            }
        }"#.split_whitespace().collect::<String>()));
});


test!(cargo_metadata_with_deps {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []
            license = "MIT"
            description = "foo"

            [[bin]]
            name = "foo"

            [dependencies]
            bar = "*"
        "#);
    Package::new("baz", "0.0.1").publish();
    Package::new("bar", "0.0.1").dep("baz", "0.0.1").publish();

    assert_that(p.cargo_process("metadata")
                 .arg("--output-path").arg("metadata.json")
                 .arg("-f").arg("json"),

                 execs().with_status(0));

    let outputfile = p.root().join("metadata.json");
    assert_that(&outputfile, existing_file());

    let mut output = String::new();
    File::open(&outputfile).unwrap().read_to_string(&mut output).unwrap();
    let result = Json::from_str(&output).unwrap();
    println!("{}", result.pretty());

    let packages = result.find("packages")
                         .and_then(|o| o.as_array())
                         .expect("no packages");

    assert_that(packages.len(), is(equal_to(3)));

    let root = result.find_path(&["resolve", "root"])
                     .and_then(|o| o.as_object())
                     .expect("no root");

    // source is null because foo is root
    let foo_id_start = format!("{} {}", root["name"].as_string().unwrap(),
                                        root["version"].as_string().unwrap());
    let foo_name = packages.iter().find(|o| {
        o.find("id").and_then(|i| i.as_string()).unwrap()
         .starts_with(&foo_id_start)
    }).and_then(|p| p.find("name"))
      .and_then(|n| n.as_string())
      .expect("no root package");
    assert_that(foo_name, is(equal_to("foo")));

    let foo_deps = root["dependencies"].as_array().expect("no root deps");
    assert_that(foo_deps.len(), is(equal_to(1)));

    let bar = &foo_deps[0].as_string().expect("bad root dep");

    let check_name_for_id = |id: &str, expected_name: &str| {
        let name = packages.iter().find(|o| {
            id == o.find("id").and_then(|i| i.as_string()).unwrap()
        }).and_then(|p| p.find("name"))
          .and_then(|n| n.as_string())
          .expect(&format!("no {} in packages", expected_name));

        assert_that(name, is(equal_to(expected_name)));
    };

    let find_deps = |id: &str| -> Vec<_> {
        result.find_path(&["resolve", "package"])
              .and_then(|o| o.as_array()).expect("resolve.package is not an array")
              .iter()
              .find(|o| {
                  let o = o.as_object().expect("package is not an object");
                  let o_id = format!("{} {} ({})",
                                     o["name"].as_string().unwrap(),
                                     o["version"].as_string().unwrap(),
                                     o["source"].as_string().unwrap());
                  id == o_id
              })
              .and_then(|o| o.find("dependencies"))
              .and_then(|o| o.as_array())
              .and_then(|a| a.iter()
                             .map(|o| o.as_string())
                             .collect())
              .expect(&format!("no deps for {}", id))
    };


    check_name_for_id(bar, "bar");
    let bar_deps = find_deps(&bar);
    assert_that(bar_deps.len(), is(equal_to(1)));

    let baz = &bar_deps[0];
    check_name_for_id(baz, "baz");
    let baz_deps = find_deps(&baz);
    assert_that(baz_deps.len(), is(equal_to(0)));
});

test!(cargo_metadata_with_invalid_manifest {
    let p = project("foo")
            .file("Cargo.toml", "");

    assert_that(p.cargo_process("metadata"), execs().with_status(101)
                                                    .with_stderr("\
failed to parse manifest at `[..]`

Caused by:
  no `package` or `project` section found."))
});

test!(cargo_metadata_with_invalid_output_format {
    let p = project("foo")
            .file("Cargo.toml", &basic_bin_manifest("foo"));

    assert_that(p.cargo_process("metadata").arg("--output-format").arg("XML"),
                execs().with_status(101)
                       .with_stderr("unknown format: XML, supported formats are TOML, JSON."))
});

test!(cargo_metadata_simple_file {
    let p = project("foo")
            .file("Cargo.toml", &basic_bin_manifest("foo"));

    assert_that(p.cargo_process("metadata").arg("--output-path").arg("metadata.toml"),
        execs().with_status(0));

    let outputfile = p.root().join("metadata.toml");
    assert_that(&outputfile, existing_file());

    let mut output = String::new();
    File::open(&outputfile).unwrap().read_to_string(&mut output).unwrap();

    assert_that(output[..].contains(r#"name = "foo""#), is(equal_to(true)));
});
