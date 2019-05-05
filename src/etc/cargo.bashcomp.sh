# Required for bash versions < 4.1
# Default bash version is 3.2 on latest macOS. See #6874
shopt -s extglob

command -v cargo >/dev/null 2>&1 &&
_cargo()
{
	local cur prev words cword
	_get_comp_words_by_ref cur prev words cword

	COMPREPLY=()

	# Skip past - and + options to find the command.
	local nwords=${#words[@]}
	local cmd_i cmd dd_i
	for (( cmd_i=1; cmd_i<$nwords; cmd_i++ ));
	do
		if [[ ! "${words[$cmd_i]}" =~ ^[+-] ]]; then
			cmd="${words[$cmd_i]}"
			break
		fi
	done
	# Find the location of the -- separator.
	for (( dd_i=1; dd_i<$nwords-1; dd_i++ ));
	do
		if [[ "${words[$dd_i]}" = "--" ]]; then
			break
		fi
	done

	local vcs='git hg none pijul fossil'
	local color='auto always never'
	local msg_format='human json short'

	local opt_help='-h --help'
	local opt_verbose='-v --verbose'
	local opt_quiet='-q --quiet'
	local opt_color='--color'
	local opt_common="$opt_help $opt_verbose $opt_quiet $opt_color"
	local opt_pkg_spec='-p --package --all --exclude'
	local opt_pkg='-p --package'
	local opt_feat='--features --all-features --no-default-features'
	local opt_mani='--manifest-path'
	local opt_jobs='-j --jobs'
	local opt_force='-f --force'
	local opt_test='--test --bench'
	local opt_lock='--frozen --locked'
	local opt_targets="--lib --bin --bins --example --examples --test --tests --bench --benches --all-targets"

	local opt___nocmd="$opt_common -V --version --list --explain"
	local opt__bench="$opt_common $opt_pkg_spec $opt_feat $opt_mani $opt_lock $opt_jobs $opt_test $opt_targets --message-format --target --no-run --no-fail-fast --target-dir"
	local opt__build="$opt_common $opt_pkg_spec $opt_feat $opt_mani $opt_lock $opt_jobs $opt_test $opt_targets --message-format --target --release --target-dir"
	local opt__check="$opt_common $opt_pkg_spec $opt_feat $opt_mani $opt_lock $opt_jobs $opt_test $opt_targets --message-format --target --release --profile --target-dir"
	local opt__clean="$opt_common $opt_pkg $opt_mani $opt_lock --target --release --doc --target-dir"
	local opt__doc="$opt_common $opt_pkg_spec $opt_feat $opt_mani $opt_lock $opt_jobs --message-format --bin --bins --lib --target --open --no-deps --release --document-private-items --target-dir"
	local opt__fetch="$opt_common $opt_mani $opt_lock"
	local opt__fix="$opt_common $opt_pkg_spec $opt_feat $opt_mani $opt_jobs $opt_targets $opt_lock --release --target --message-format --prepare-for --broken-code --edition --edition-idioms --allow-no-vcs --allow-dirty --allow-staged --profile --target-dir"
	local opt__generate_lockfile="${opt__fetch}"
	local opt__git_checkout="$opt_common $opt_lock --reference --url"
	local opt__help="$opt_help"
	local opt__init="$opt_common $opt_lock --bin --lib --name --vcs --edition --registry"
	local opt__install="$opt_common $opt_feat $opt_jobs $opt_lock $opt_force --bin --bins --branch --debug --example --examples --git --list --path --rev --root --tag --version --registry --target"
	local opt__locate_project="$opt_mani -h --help"
	local opt__login="$opt_common $opt_lock --host --registry"
	local opt__metadata="$opt_common $opt_feat $opt_mani $opt_lock --format-version=1 --no-deps"
	local opt__new="$opt_common $opt_lock --vcs --bin --lib --name --edition --registry"
	local opt__owner="$opt_common $opt_lock -a --add -r --remove -l --list --index --token --registry"
	local opt__package="$opt_common $opt_mani $opt_feat $opt_lock $opt_jobs --allow-dirty -l --list --no-verify --no-metadata --target --target-dir"
	local opt__pkgid="${opt__fetch} $opt_pkg"
	local opt__publish="$opt_common $opt_mani $opt_feat $opt_lock $opt_jobs --allow-dirty --dry-run --host --token --no-verify --index --registry --target --target-dir"
	local opt__read_manifest="$opt_help $opt_quiet $opt_verbose $opt_mani $opt_color "
	local opt__run="$opt_common $opt_pkg $opt_feat $opt_mani $opt_lock $opt_jobs --message-format --target --bin --example --release --target-dir"
	local opt__rustc="$opt_common $opt_pkg $opt_feat $opt_mani $opt_lock $opt_jobs $opt_test $opt_targets --message-format --profile --target --release --target-dir"
	local opt__rustdoc="$opt_common $opt_pkg $opt_feat $opt_mani $opt_lock $opt_jobs $opt_test $opt_targets --message-format --target --release --open --target-dir"
	local opt__search="$opt_common $opt_lock --host --limit --index --limit --registry"
	local opt__test="$opt_common $opt_pkg_spec $opt_feat $opt_mani $opt_lock $opt_jobs $opt_test $opt_targets --message-format --doc --target --no-run --release --no-fail-fast --target-dir"
	local opt__uninstall="$opt_common $opt_lock $opt_pkg_spec --bin --root"
	local opt__update="$opt_common $opt_pkg_spec $opt_mani $opt_lock --aggressive --precise --dry-run"
	local opt__verify_project="${opt__fetch}"
	local opt__version="$opt_help $opt_verbose $opt_color"
	local opt__yank="$opt_common $opt_lock --vers --undo --index --token --registry"
	local opt__libtest="--help --include-ignored --ignored --test --bench --list --logfile --nocapture --test-threads --skip -q --quiet --exact --color --format"

	if [[ $cword -gt $dd_i ]]; then
		# Completion after -- separator.
		if [[ "${cmd}" = @(test|bench) ]]; then
			COMPREPLY=( $( compgen -W "${opt__libtest}" -- "$cur" ) )
		else
			# Fallback to filename completion, useful with `cargo run`.
			_filedir
		fi
	elif [[ $cword -le $cmd_i ]]; then
		# Completion before or at the command.
		if [[ "$cur" == -* ]]; then
			COMPREPLY=( $( compgen -W "${opt___nocmd}" -- "$cur" ) )
		elif [[ "$cur" == +* ]]; then
			COMPREPLY=( $( compgen -W "$(_toolchains)" -- "$cur" ) )
		else
			COMPREPLY=( $( compgen -W "$__cargo_commands" -- "$cur" ) )
		fi
	else
		case "${prev}" in
			--vcs)
				COMPREPLY=( $( compgen -W "$vcs" -- "$cur" ) )
				;;
			--color)
				COMPREPLY=( $( compgen -W "$color" -- "$cur" ) )
				;;
			--message-format)
				COMPREPLY=( $( compgen -W "$msg_format" -- "$cur" ) )
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
			--target-dir)
				_filedir -d
				;;
			help)
				COMPREPLY=( $( compgen -W "$__cargo_commands" -- "$cur" ) )
				;;
			*)
				local opt_var=opt__${cmd//-/_}
				if [[ -z "${!opt_var}" ]]; then
					# Fallback to filename completion.
					_filedir
				else
					COMPREPLY=( $( compgen -W "${!opt_var}" -- "$cur" ) )
				fi
				;;
		esac
	fi

	# compopt does not work in bash version 3

	return 0
} &&
complete -F _cargo cargo

__cargo_commands=$(cargo --list 2>/dev/null | awk 'NR>1 {print $1}')

_locate_manifest(){
	local manifest=`cargo locate-project 2>/dev/null`
	# regexp-replace manifest '\{"root":"|"\}' ''
	echo ${manifest:9:${#manifest}-11}
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
	local manifest=$(_locate_manifest)
	[ -z "$manifest" ] && return 0

	local files=("${manifest%/*}"/examples/*.rs)
	local names=("${files[@]##*/}")
	local names=("${names[@]%.*}")
	# "*" means no examples found
	if [[ "${names[@]}" != "*" ]]; then
		echo "${names[@]}"
	fi
}

_get_targets(){
	local result=()
	local targets=$(rustup target list)
	while read line
	do
		if [[ "$line" =~ default|installed ]]; then
			result+=("${line%% *}")
		fi
	done <<< "$targets"
	echo "${result[@]}"
}

_toolchains(){
	local result=()
	local toolchains=$(rustup toolchain list)
	local channels="nightly|beta|stable|[0-9]\.[0-9]{1,2}\.[0-9]"
	local date="[0-9]{4}-[0-9]{2}-[0-9]{2}"
	while read line
	do
		# Strip " (default)"
		line=${line%% *}
		if [[ "$line" =~ ^($channels)(-($date))?(-.*) ]]; then
			if [[ -z ${BASH_REMATCH[3]} ]]; then
				result+=("+${BASH_REMATCH[1]}")
			else
				# channel-date
				result+=("+${BASH_REMATCH[1]}-${BASH_REMATCH[3]}")
			fi
			result+=("+$line")
		else
			result+=("+$line")
		fi
	done <<< "$toolchains"
	echo "${result[@]}"
}

# vim:ft=sh
