command -v cargo >/dev/null 2>&1 &&
_cargo()
{
	local cur prev words cword cmd
	_get_comp_words_by_ref cur prev words cword

	COMPREPLY=()

	cmd=${words[1]}

	local vcs='git hg none'
	local color='auto always never'

	local opt_help='-h --help'
	local opt_verbose='-v --verbose'
	local opt_quiet='-q --quiet'
	local opt_color='--color'
	local opt_common="$opt_help $opt_verbose $opt_quiet $opt_color"
	local opt_pkg='-p --package'
	local opt_feat='--features --no-default-features'
	local opt_mani='--manifest-path'
	local opt_jobs='-j --jobs'

	local opt___nocmd="$opt_common -V --version --list"
	local opt__bench="$opt_common $opt_pkg $opt_feat $opt_mani $opt_jobs --target --lib --bin --test --bench --example --no-run"
	local opt__build="$opt_common $opt_pkg $opt_feat $opt_mani $opt_jobs --target --lib --bin --test --bench --example --release"
	local opt__clean="$opt_common $opt_pkg $opt_mani --target --release"
	local opt__doc="$opt_common $opt_pkg $opt_feat $opt_mani $opt_jobs --target --open --no-deps --release"
	local opt__fetch="$opt_common $opt_mani"
	local opt__generate_lockfile="${opt__fetch}"
	local opt__git_checkout="$opt_common --reference --url"
	local opt__help="$opt_help"
	local opt__init="$opt_common --bin --name --vcs"
	local opt__install="$opt_common $opt_feat $opt_jobs --bin --branch --debug --example --git --list --path --rev --root --tag --vers"
	local opt__locate_project="$opt_mani -h --help"
	local opt__login="$opt_common --host"
	local opt__metadata="$opt_common $opt_feat $opt_mani --format-version"
	local opt__new="$opt_common --vcs --bin --name"
	local opt__owner="$opt_common -a --add -r --remove -l --list --index --token"
	local opt__package="$opt_common $opt_mani -l --list --no-verify --no-metadata"
	local opt__pkgid="${opt__fetch}"
	local opt__publish="$opt_common $opt_mani --host --token --no-verify"
	local opt__read_manifest="$opt_help $opt_verbose $opt_mani $opt_color"
	local opt__run="$opt_common $opt_feat $opt_mani $opt_jobs --target --bin --example --release"
	local opt__rustc="$opt_common $opt_pkg $opt_feat $opt_mani $opt_jobs --target --lib --bin --test --bench --example --release"
	local opt__rustdoc="$opt_common $opt_pkg $opt_feat $opt_mani $opt_jobs --target --lib --bin --test --bench --example --release --open"
	local opt__search="$opt_common --host"
	local opt__test="$opt_common $opt_pkg $opt_feat $opt_mani $opt_jobs --target --lib --bin --test --bench --example --no-run --release --no-fail-fast"
	local opt__uninstall="$opt_common --bin --root"
	local opt__update="$opt_common $opt_pkg $opt_mani --aggressive --precise"
	local opt__verify_project="${opt__fetch}"
	local opt__version="$opt_help $opt_verbose $opt_color"
	local opt__yank="$opt_common --vers --undo --index --token"

	if [[ $cword -eq 1 ]]; then
		if [[ "$cur" == -* ]]; then
			COMPREPLY=( $( compgen -W "${opt___nocmd}" -- "$cur" ) )
		else
			COMPREPLY=( $( compgen -W "$__cargo_commands" -- "$cur" ) )
		fi
	elif [[ $cword -ge 2 ]]; then
		case "${prev}" in
			--vcs)
				COMPREPLY=( $( compgen -W "$vcs" -- "$cur" ) )
				;;
			--color)
				COMPREPLY=( $( compgen -W "$color" -- "$cur" ) )
				;;
			--manifest-path)
				_filedir toml
				;;
			--bin)
				COMPREPLY=( $( compgen -W "$(_bin_names)" -- "$cur" ) )
				;;
			--test)
				COMPREPLY=( $( compgen -W "$(_test_names)" -- "$cur" ) )
				;;
			--bench)
				COMPREPLY=( $( compgen -W "$(_benchmark_names)" -- "$cur" ) )
				;;
			--example)
				COMPREPLY=( $( compgen -W "$(_get_examples)" -- "$cur" ) )
				;;
			--target)
				COMPREPLY=( $( compgen -W "$(_get_targets)" -- "$cur" ) )
				;;
			help)
				COMPREPLY=( $( compgen -W "$__cargo_commands" -- "$cur" ) )
				;;
			*)
				local opt_var=opt__${cmd//-/_}
				COMPREPLY=( $( compgen -W "${!opt_var}" -- "$cur" ) )
				;;
		esac
	fi

	# compopt does not work in bash version 3

	return 0
} &&
complete -F _cargo cargo

__cargo_commands=$(cargo --list 2>/dev/null | tail -n +2)

_locate_manifest(){
	local manifest=`cargo locate-project 2>/dev/null`
	# regexp-replace manifest '\{"root":"|"\}' ''
	echo ${manifest:9:-2}
}

# Extracts the values of "name" from the array given in $1 and shows them as
# command line options for completion
_get_names_from_array()
{
    local manifest=$(_locate_manifest)
    if [[ -z $manifest ]]; then
        return 0
    fi

    local last_line
    local -a names
    local in_block=false
    local block_name=$1
    while read line
    do
        if [[ $last_line == "[[$block_name]]" ]]; then
            in_block=true
        else
            if [[ $last_line =~ .*\[\[.* ]]; then
                in_block=false
            fi
        fi

        if [[ $in_block == true ]]; then
            if [[ $line =~ .*name.*\= ]]; then
                line=${line##*=}
                line=${line%%\"}
                line=${line##*\"}
                names+=($line)
            fi
        fi

        last_line=$line
    done < $manifest
    echo "${names[@]}"
}

#Gets the bin names from the manifest file
_bin_names()
{
    _get_names_from_array "bin"
}

#Gets the test names from the manifest file
_test_names()
{
    _get_names_from_array "test"
}

#Gets the bench names from the manifest file
_benchmark_names()
{
    _get_names_from_array "bench"
}

_get_examples(){
	local files=($(dirname $(_locate_manifest))/examples/*.rs)
	local names=("${files[@]##*/}")
	local names=("${names[@]%.*}")
	# "*" means no examples found
	if [[ "${names[@]}" != "*" ]]; then
		echo "${names[@]}"
	fi
}

_get_targets(){
	local CURRENT_PATH
	if [ `uname -o` == "Cygwin" -a -f "$PWD"/Cargo.toml ]; then
		CURRENT_PATH=$PWD
	else
		CURRENT_PATH=$(_locate_manifest)
	fi
	if [[ -z "$CURRENT_PATH" ]]; then
		return 1
	fi
	local TARGETS=()
	local FIND_PATHS=( "/" )
	local FIND_PATH LINES LINE
	while [[ "$CURRENT_PATH" != "/" ]]; do
	    FIND_PATHS+=( "$CURRENT_PATH" )
	    CURRENT_PATH=$(dirname $CURRENT_PATH)
	done
	for FIND_PATH in ${FIND_PATHS[@]}; do
	    if [[ -f "$FIND_PATH"/.cargo/config ]]; then
		LINES=( `grep "$FIND_PATH"/.cargo/config -e "^\[target\."` )
		for LINE in ${LINES[@]}; do
		    TARGETS+=(`sed 's/^\[target\.\(.*\)\]$/\1/' <<< $LINE`)
		done
	    fi
	done
	echo "${TARGETS[@]}"
}
# vim:ft=sh
