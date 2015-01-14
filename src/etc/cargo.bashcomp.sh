have cargo &&
_cargo() {
    local commands split=false
    local cur cmd

    COMPREPLY=()
    cur=$(_get_cword "=")
    cmd="${COMP_WORDS[1]}"

    _expand || return 0

    commands=$(cargo --list|grep -v 'Installed Commands:')

    # these options require an argument
    if [[ "${cmd}" == -@(A|B|C|G|g|m) ]] ; then
        return 0
    fi

    _split_longopt && split=true

    case "${cmd}" in
        bench|test)
            COMPREPLY=( $(compgen -W \
                "--features --help --jobs --manifest-path --no-default-features \
                 --no-run --package --target --verbose" \
                 -- "${cur}") )
            return 0;;
        build)
            COMPREPLY=( $(compgen -W \
                "--features --help --jobs --manifest-path --no-default-features \
                 --package --release --target --verbose" \
                 -- "${cur}") )
            return 0;;
        clean)
            COMPREPLY=( $(compgen -W \
                "--help --manifest-path --package --target --verbose" \
                 -- "${cur}") )
            return 0;;
        config-for-key)
            COMPREPLY=( $(compgen -W \
                "--help --human --key=" \
                 -- "${cur}") )
            return 0;;
        config-for-key)
            COMPREPLY=( $(compgen -W \
                "--help --human" \
                 -- "${cur}") )
            return 0;;
        doc)
            COMPREPLY=( $(compgen -W \
                "--features --help --jobs --manifest-path --no-default-features \
                 --no-deps --open --verbose" \
                 -- "${cur}") )
            return 0;;
        fetch|generate-lockfile|package|pkgid|read-manifest|verify-project)
            COMPREPLY=( $(compgen -W \
                "--help --manifest-path --verbose" \
                 -- "${cur}") )
            return 0;;
        git-checkout)
            COMPREPLY=( $(compgen -W \
                "--help --reference= --url= --verbose" \
                 -- "${cur}") )
            return 0;;
        locate-project)
            COMPREPLY=( $(compgen -W \
                "--help --host --verbose" \
                 -- "${cur}") )
            return 0;;
        new)
            COMPREPLY=( $(compgen -W \
                "--bin --git --help --hg --no-git --verbose" \
                 -- "${cur}") )
            return 0;;
        run)
            COMPREPLY=( $(compgen -W \
                "--features --help --jobs --manifest-path --no-default-features \
                 --release --target --verbose" \
                 -- "${cur}") )
            return 0;;
        update)
            COMPREPLY=( $(compgen -W \
                "--aggressive --help --manifest-path --package --precise --verbose" \
                 -- "${cur}") )
            return 0;;
        upload)
            COMPREPLY=( $(compgen -W \
                "--help --host --manifest-path --token --verbose" \
                 -- "${cur}") )
            return 0;;
        version)
            COMPREPLY=( $(compgen -W \
                "--help --verbose" \
                 -- "${cur}") )
            return 0;;
        help)
            return 0;;
    esac

    $split && return 0

    if [ ${COMP_CWORD} -eq 1 ]; then
        COMPREPLY=( $(compgen -W \
            "${commands} --help --list --verbose --version" -- "${cur}") )
        return 0
    fi
    _filedir
} &&
complete -F _cargo ${nospace} cargo
