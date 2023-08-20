# Git Authentication

Cargo supports some forms of authentication when using git dependencies and
registries. This appendix contains some information for setting up git
authentication in a way that works with Cargo.

If you need other authentication methods, the [`net.git-fetch-with-cli`]
config value can be set to cause Cargo to execute the `git` executable to
handle fetching remote repositories instead of using the built-in support.
This can be enabled with the `CARGO_NET_GIT_FETCH_WITH_CLI=true` environment
variable.

## HTTPS authentication

HTTPS authentication requires the [`credential.helper`] mechanism. There are
multiple credential helpers, and you specify the one you want to use in your
global git configuration file.

```ini
# ~/.gitconfig

[credential]
helper = store
```

Cargo does not ask for passwords, so for most helpers you will need to give
the helper the initial username/password before running Cargo. One way to do
this is to run `git clone` of the private git repo and enter the
username/password.

> **Tip:**<br>
> macOS users may want to consider using the osxkeychain helper.<br>
> Windows users may want to consider using the [GCM] helper.

> **Note:** Windows users will need to make sure that the `sh` shell is
> available in your `PATH`. This typically is available with the Git for
> Windows installation.

## SSH authentication

SSH authentication requires `ssh-agent` to be running to acquire the SSH key.
Make sure the appropriate environment variables are set up (`SSH_AUTH_SOCK` on
most Unix-like systems), and that the correct keys are added (with `ssh-add`).

Windows can use Pageant (part of [PuTTY]) or `ssh-agent`.
To use `ssh-agent`, Cargo needs to use the OpenSSH that is distributed as part
of Windows, as Cargo does not support the simulated Unix-domain sockets used
by MinGW or Cygwin.
More information about installing with Windows can be found at the [Microsoft
installation documentation] and the page on [key management] has instructions
on how to start `ssh-agent` and to add keys.

> **Note:** Cargo does not support git's shorthand SSH URLs like
> `git@example.com:user/repo.git`. Use a full SSH URL like
> `ssh://git@example.com/user/repo.git`.

> **Note:** SSH configuration files (like OpenSSH's `~/.ssh/config`) are not
> used by Cargo's built-in SSH library. More advanced requirements should use
> [`net.git-fetch-with-cli`].

### SSH Known Hosts

When connecting to an SSH host, Cargo must verify the identity of the host
using "known hosts", which are a list of host keys. Cargo can look for these
known hosts in OpenSSH-style `known_hosts` files located in their standard
locations (`.ssh/known_hosts` in your home directory, or
`/etc/ssh/ssh_known_hosts` on Unix-like platforms or
`%PROGRAMDATA%\ssh\ssh_known_hosts` on Windows). More information about these
files can be found in the [sshd man page]. Alternatively, keys may be
configured in a Cargo configuration file with [`net.ssh.known-hosts`].

When connecting to an SSH host before the known hosts has been configured,
Cargo will display an error message instructing you how to add the host key.
This also includes a "fingerprint", which is a smaller hash of the host key,
which should be easier to visually verify. The server administrator can get
the fingerprint by running `ssh-keygen` against the public key (for example,
`ssh-keygen -l -f /etc/ssh/ssh_host_ecdsa_key.pub`). Well-known sites may
publish their fingerprints on the web; for example GitHub posts theirs at
<https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/githubs-ssh-key-fingerprints>.

Cargo comes with the host keys for [github.com](https://github.com) built-in.
If those ever change, you can add the new keys to the config or known_hosts file.

> **Note:** Cargo doesn't support the `@cert-authority` or `@revoked`
> markers in `known_hosts` files. To make use of this functionality, use
> [`net.git-fetch-with-cli`]. This is also a good tip if Cargo's SSH client
> isn't behaving the way you expect it to.

[`credential.helper`]: https://git-scm.com/book/en/v2/Git-Tools-Credential-Storage
[`net.git-fetch-with-cli`]: ../reference/config.md#netgit-fetch-with-cli
[`net.ssh.known-hosts`]: ../reference/config.md#netsshknown-hosts
[GCM]: https://github.com/microsoft/Git-Credential-Manager-Core/
[PuTTY]: https://www.chiark.greenend.org.uk/~sgtatham/putty/
[Microsoft installation documentation]: https://docs.microsoft.com/en-us/windows-server/administration/openssh/openssh_install_firstuse
[key management]: https://docs.microsoft.com/en-us/windows-server/administration/openssh/openssh_keymanagement
[sshd man page]: https://man.openbsd.org/sshd#SSH_KNOWN_HOSTS_FILE_FORMAT
