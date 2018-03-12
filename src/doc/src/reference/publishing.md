## Publishing on crates.io

Once you've got a library that you'd like to share with the world, it's time to
publish it on [crates.io]! Publishing a crate is when a specific
version is uploaded to be hosted on [crates.io].

Take care when publishing a crate, because a publish is **permanent**. The
version can never be overwritten, and the code cannot be deleted. There is no
limit to the number of versions which can be published, however.

### Before your first publish

First thing’s first, you’ll need an account on [crates.io] to acquire
an API token. To do so, [visit the home page][crates.io] and log in via a GitHub
account (required for now). After this, visit your [Account
Settings](https://crates.io/me) page and run the `cargo login` command
specified.

```console
$ cargo login abcdefghijklmnopqrstuvwxyz012345
```

This command will inform Cargo of your API token and store it locally in your
`~/.cargo/credentials` (previously it was `~/.cargo/config`).  Note that this
token is a **secret** and should not be shared with anyone else. If it leaks for
any reason, you should regenerate it immediately.

### Before publishing a new crate

Keep in mind that crate names on [crates.io] are allocated on a first-come-first-
serve basis. Once a crate name is taken, it cannot be used for another crate.

#### Packaging a crate

The next step is to package up your crate into a format that can be uploaded to
[crates.io]. For this we’ll use the `cargo package` subcommand. This will take
our entire crate and package it all up into a `*.crate` file in the
`target/package` directory.

```console
$ cargo package
```

As an added bonus, the `*.crate` will be verified independently of the current
source tree. After the `*.crate` is created, it’s unpacked into
`target/package` and then built from scratch to ensure that all necessary files
are there for the build to succeed. This behavior can be disabled with the
`--no-verify` flag.

Now’s a good time to take a look at the `*.crate` file to make sure you didn’t
accidentally package up that 2GB video asset, or large data files used for code
generation, integration tests, or benchmarking.  There is currently a 10MB
upload size limit on `*.crate` files. So, if the size of `tests` and `benches`
directories and their dependencies are up to a couple of MBs, you can keep them
in your package; otherwise, better to exclude them.

Cargo will automatically ignore files ignored by your version control system
when packaging, but if you want to specify an extra set of files to ignore you
can use the `exclude` key in the manifest:

```toml
[package]
# ...
exclude = [
    "public/assets/*",
    "videos/*",
]
```

The syntax of each element in this array is what
[rust-lang/glob](https://github.com/rust-lang/glob) accepts. If you’d rather
roll with a whitelist instead of a blacklist, Cargo also supports an `include`
key, which if set, overrides the `exclude` key:

```toml
[package]
# ...
include = [
    "**/*.rs",
    "Cargo.toml",
]
```

### Uploading the crate

Now that we’ve got a `*.crate` file ready to go, it can be uploaded to
[crates.io] with the `cargo publish` command. And that’s it, you’ve now published
your first crate!

```console
$ cargo publish
```

If you’d like to skip the `cargo package` step, the `cargo publish` subcommand
will automatically package up the local crate if a copy isn’t found already.

Be sure to check out the [metadata you can
specify](reference/manifest.html#package-metadata) to ensure your crate can be
discovered more easily!

### Publishing a new version of an existing crate

In order to release a new version, change the `version` value specified in your
`Cargo.toml` manifest. Keep in mind [the semver
rules](reference/manifest.html#the-version-field). Then optionally run `cargo package` if
you want to inspect the `*.crate` file for the new version before publishing,
and run `cargo publish` to upload the new version.

### Managing a crates.io-based crate

Management of crates is primarily done through the command line `cargo` tool
rather than the [crates.io] web interface. For this, there are a few subcommands
to manage a crate.

#### `cargo yank`

Occasions may arise where you publish a version of a crate that actually ends up
being broken for one reason or another (syntax error, forgot to include a file,
etc.). For situations such as this, Cargo supports a “yank” of a version of a
crate.

```console
$ cargo yank --vers 1.0.1
$ cargo yank --vers 1.0.1 --undo
```

A yank **does not** delete any code. This feature is not intended for deleting
accidentally uploaded secrets, for example. If that happens, you must reset
those secrets immediately.

The semantics of a yanked version are that no new dependencies can be created
against that version, but all existing dependencies continue to work. One of the
major goals of [crates.io] is to act as a permanent archive of crates that does
not change over time, and allowing deletion of a version would go against this
goal. Essentially a yank means that all projects with a `Cargo.lock` will not
break, while any future `Cargo.lock` files generated will not list the yanked
version.

#### `cargo owner`

A crate is often developed by more than one person, or the primary maintainer
may change over time! The owner of a crate is the only person allowed to publish
new versions of the crate, but an owner may designate additional owners.

```console
$ cargo owner --add my-buddy
$ cargo owner --remove my-buddy
$ cargo owner --add github:rust-lang:owners
$ cargo owner --remove github:rust-lang:owners
```

The owner IDs given to these commands must be GitHub user names or GitHub teams.

If a user name is given to `--add`, that user becomes a “named” owner, with
full rights to the crate. In addition to being able to publish or yank versions
of the crate, they have the ability to add or remove owners, *including* the
owner that made *them* an owner. Needless to say, you shouldn’t make people you
don’t fully trust into a named owner. In order to become a named owner, a user
must have logged into [crates.io] previously.

If a team name is given to `--add`, that team becomes a “team” owner, with
restricted right to the crate. While they have permission to publish or yank
versions of the crate, they *do not* have the ability to add or remove owners.
In addition to being more convenient for managing groups of owners, teams are
just a bit more secure against owners becoming malicious.

The syntax for teams is currently `github:org:team` (see examples above).
In order to add a team as an owner one must be a member of that team. No
such restriction applies to removing a team as an owner.

#### `cargo publish-build-info`

The `cargo publish-build-info` command is intended to help automate reporting
on which versions of Rust your crate's released versions work successfully
with. It is meant to work with the results of continuous integration runs. It
will work with any CI service; below are instructions for Travis CI, but the
idea should be generalizable to any setup.

`cargo publish-build-info` will report the version of rustc, the version of
your crate, and the target that you run the command with. The target may
optionally be specified as something other than the operating system the
command is running on by specifying the `--target` flag.

When CI runs on a tagged (released) version of your crate, run this command
with the value `pass` or `fail` depending on the results of your CI script.

Results with a particular crate version, rustc version, and target can only be
reported once. A possible enhancement is to allow overwriting in the future.
Until then, only report on your final tagged release version.

Crates.io must already know about a crate and version in order for you to
publish build information about them, so intended workflow is:

1. Run regular CI to verify your crate compiles and passes tests
2. Bump to the version you want to release in `Cargo.toml` and commit
3. Tag that commit since the CI setup recommended below will only run on tagged
   versions
4. Publish to crates.io
5. Push the tag in order to run CI on the tagged version, which will then run
   `cargo publish-build-info` with the results.

Yes, you can report possibly-incorrect results manually, but your users
will probably report a bug if they can't reproduce your reported results.

On crate list pages such as search results, your crate will have a badge if you
have reported `pass` results for the max version of your crate. If you have
reported that it passes on stable, the version of stable will be displayed in a
green badge. If no stable versions have a reported pass result, but a beta
version of Rust has, the date of the latest beta that passed will be displayed
in a yellow badge. If there have been no pass results on stable or beta but
there have been for nightly, the date of the latest nightly that passed will be
displayed in an orange badge. If there have been no pass results reported for
any Rust version, no badge will be shown for that crate.

If there have been any results reported for the Tier 1 targets on 64 bit
architectures for a version of a crate, there will be a section on that
version's page titled "Build info" that will display more detailed results for
the latest version of each of the stable, beta, and nightly channels for those
targets.

##### Travis configuration to automatically report build info

First, make an [encrypted environment variable][travis-env] named TOKEN with
your crates.io API key.

Then add this to your `.travis.yml`, substituting in your secure environment
variable value where indicated:

```yml
env:
  - secure: [your secure env var value here]

after_script: >
  if [ -n "$TRAVIS_TAG" ] ; then
     result=$([[ $TRAVIS_TEST_RESULT = 0 ]] && echo pass || echo fail)
     cargo publish-build-info $result --token TOKEN
  fi
```

The code in `after_script` checks to see if you're building a tagged commit,
and if so, checks to see if the build passed or failed, then runs the `cargo
publish-build-info` command to send the build result to crates.io.

[travis-env]: https://docs.travis-ci.com/user/environment-variables/#Defining-encrypted-variables-in-.travis.yml

### GitHub permissions

Team membership is not something GitHub provides simple public access to, and it
is likely for you to encounter the following message when working with them:

> It looks like you don’t have permission to query a necessary property from
GitHub to complete this request. You may need to re-authenticate on [crates.io]
to grant permission to read GitHub org memberships. Just go to
https://crates.io/login

This is basically a catch-all for “you tried to query a team, and one of the
five levels of membership access control denied this”. That is not an
exaggeration. GitHub’s support for team access control is Enterprise Grade.

The most likely cause of this is simply that you last logged in before this
feature was added. We originally requested *no* permissions from GitHub when
authenticating users, because we didn’t actually ever use the user’s token for
anything other than logging them in. However to query team membership on your
behalf, we now require
[the `read:org` scope](https://developer.github.com/v3/oauth/#scopes).

You are free to deny us this scope, and everything that worked before teams
were introduced will keep working. However you will never be able to add a team
as an owner, or publish a crate as a team owner. If you ever attempt to do this,
you will get the error above. You may also see this error if you ever try to
publish a crate that you don’t own at all, but otherwise happens to have a team.

If you ever change your mind, or just aren’t sure if [crates.io] has sufficient
permission, you can always go to https://crates.io/login, which will prompt you
for permission if [crates.io] doesn’t have all the scopes it would like to.

An additional barrier to querying GitHub is that the organization may be
actively denying third party access. To check this, you can go to:

    https://github.com/organizations/:org/settings/oauth_application_policy

where `:org` is the name of the organization (e.g. rust-lang). You may see
something like:

![Organization Access Control](images/org-level-acl.png)

Where you may choose to explicitly remove [crates.io] from your organization’s
blacklist, or simply press the “Remove Restrictions” button to allow all third
party applications to access this data.

Alternatively, when [crates.io] requested the `read:org` scope, you could have
explicitly whitelisted [crates.io] querying the org in question by pressing
the “Grant Access” button next to its name:

![Authentication Access Control](images/auth-level-acl.png)

[crates.io]: https://crates.io/
