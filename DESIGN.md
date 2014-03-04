## Subcommands

The top-level `cargo` command delegates to sub-commands named
`cargo-foo`.

```
$ cargo compile
# delegates to cargo-compile
```

By default, Cargo will come with a set of built-in commands that don't
need to be on the `$PATH`, but additional commands can be added to the
`$PATH`.

There will also be an additional configuration for locating Cargo
subcommands that are not on the `$PATH`.

### Input/Output

By default, Cargo subcommands are built by implementing the
`CargoCommand` trait. This trait will pass structured data to the
command.

By default, commands will communicate with each other via JSON data, and
the `CargoCommand` trait will convert the JSON data into the structured
data needed by the command. All commands must implement JSON data
output.

Commands must also implement human-readable output, and may implement
additional output forms (such as tab- or space-separated output) for use
in other scripting languages.

```rs
// The main entry point for new commands to implement
trait CargoCommand<T, U> {
  fn execute<L: CargoLogger>(input: T, logger: L) -> Result<U, CargoErr>;
}

// For now, the raw IPC communication is represented as JSON primitive
// values. The ConvertToRaw trait below converts a string protocol into the
// Raw format. Obviously, the JSON string protocol can trivially be
// converted, but other line protocols need to be defined on a
// case-by-case basis.
type Raw = serialize::json::Json;
type Flags = Map<~str, serialize::json::Json>

// This is a list of available IPC String protocols. To start, we'll
// support JSON and an (optional) arbitrary type-defined line protocol.
enum Input {
  JSONString(~str),
  LineOrientedString(~str)
}

// This trait supports converting any supported input form into Raw.
trait ConvertToRaw<Input> {
  fn convert(input: Input) -> Raw;
}

// This is the runner implementation. It will not need to be implemented
// by individual commands.
fn execute_command<Config, Output, C: CargoCommand, L: Logger>(command: C, config: Config, logger: L) -> Output {
  match command.execute(input, logger) {
    Ok(success) => {
      // serialize success
    },
    Err(failure) => {
      // error handling/output
    }
  }
}

// This is an example configuration. It is the combination of the Raw
// input from a previous command and any flags passed to this command.
// Top-level commands will mostly be configured via flags -- plumbing
// commands will be mostly configured via Raw.
//
// Note that because configurations serve as both input and output, and
// the ConvertToRaw trait handles both sides of the pipe, these definitions
// are not part of an individual command. Some configuration structures
// may even be used by multiple different commands.
struct CompileConfig {
  flags: ~[~str],
  path: ~[~str],
  lib_path: ~str
}

struct CompileConfigBuilder {
  flags: Option<~[~str]>,
  path: Option<~[~str]>,
  lib_path: Option<~str>
}

// For now, configurations manually convert the Flags and Raw into a
// single configuration object. This is the main point where a failure
// can occur that is not type-checked. All other functions receive the
// structured type and will get compiler help.
impl CompileConfig {
  pub fn deserialize(flags: Flags, raw: Raw) -> CompileConfig {
    CompileConfig{ flags: raw.at("flags"), path: raw.at("path"), lib_path: flags.at("lib_path") }
  }
}

// Configurations must implement ConvertIntoRaw<JSONString> and may
// implement other ConvertIntoRaw converters.
impl ConvertToRaw<JSONString> for CompileConfig {
  fn convert(input: JSONString) -> Raw {

  }
}

impl ConvertToRaw<LineOrientedString> for CompileConfig {
  fn convert(input: LineOrientedString) -> Raw {

  }
}

impl ConvertFlags for CompileConfig {
  fn convert(input: FlagDefinition) -> Flags {

  }
}

// Commands are simple objects that implement CargoCommand for a given
struct CompileCommand;

impl CompileCommand {
  fn new() -> CompileCommand { CompileCommand }
}

impl CargoCommand<CompileConfig, CompileOutput> for CompileCommand {
  fn execute<L: CargoLogger>(input: CompileConfig, logger: L) -> Result<CompileOutput, CargoErr>;

  }
}

fn main() {
  let args = parse_arguments(f);
  let config = process_args_and_stdin(args); // { "flags": [ ... ] }
  let command = CompileCommand::new()
  let logger = CargoLogger::for(config);
  let result = execute_command(command, config, logger);

  // deal with serialized output or error
}

fn process_args_and_stdin(args: Flags) -> CompileConfig {
  // delegate to other generic function; Flags tells us which serializer
  // to use
}
```

## Configuration
