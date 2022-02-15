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

[`credential.helper`]: https://git-scm.com/book/en/v2/Git-Tools-Credential-Storage
[`net.git-fetch-with-cli`]: ../reference/config.md#netgit-fetch-with-cli
[GCM]: https://github.com/microsoft/Git-Credential-Manager-Core/
[PuTTY]: https://www.chiark.greenend.org.uk/~sgtatham/putty/
[Microsoft installation documentation]: https://docs.microsoft.com/en-us/windows-server/administration/openssh/openssh_install_firstuse
[key management]: https://docs.microsoft.com/en-us/windows-server/administration/openssh/openssh_keymanagement
