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
	local opt_pkg_spec='-p --package --all --exclude --workspace'
	local opt_pkg='-p --package'
	local opt_feat='-F --features --all-features --no-default-features'
	local opt_mani='--manifest-path'
	local opt_jobs='-j --jobs'
	local opt_parallel="$opt_jobs --keep-going"
	local opt_force='-f --force'
	local opt_sync='-s --sync'
	local opt_lock='--frozen --locked --offline'
	local opt_targets="--lib --bin --bins --example --examples --test --tests --bench --benches --all-targets"

	local opt___nocmd="$opt_common -V --version --list --explain"
	local opt__add="$opt_common -p --package --features --default-features --no-default-features $opt_mani --optional --no-optional --rename --dry-run --path --git --branch --tag --rev --registry --dev --build --target"
	local opt__bench="$opt_common $opt_pkg_spec $opt_feat $opt_mani $opt_lock $opt_jobs $opt_targets --message-format --target --no-run --no-fail-fast --target-dir --ignore-rust-version"
	local opt__build="$opt_common $opt_pkg_spec $opt_feat $opt_mani $opt_lock $opt_parallel $opt_targets --message-format --target --release --profile --target-dir --ignore-rust-version"
	local opt__b="$opt__build"
	local opt__check="$opt_common $opt_pkg_spec $opt_feat $opt_mani $opt_lock $opt_parallel $opt_targets --message-format --target --release --profile --target-dir --ignore-rust-version"
	local opt__c="$opt__check"
	local opt__clean="$opt_common $opt_pkg $opt_mani $opt_lock --target --release --doc --target-dir --profile"
	local opt__clippy="$opt_common $opt_pkg_spec $opt_feat $opt_mani $opt_lock $opt_parallel $opt_targets --message-format --target --release --profile --target-dir --no-deps --fix"
	local opt__doc="$opt_common $opt_pkg_spec $opt_feat $opt_mani $opt_lock $opt_parallel --message-format --bin --bins --lib --target --open --no-deps --release --document-private-items --target-dir --profile --ignore-rust-version"
	local opt__d="$opt__doc"
	local opt__fetch="$opt_common $opt_mani $opt_lock --target"
	local opt__fix="$opt_common $opt_pkg_spec $opt_feat $opt_mani $opt_parallel $opt_targets $opt_lock --release --target --message-format --broken-code --edition --edition-idioms --allow-no-vcs --allow-dirty --allow-staged --profile --target-dir --ignore-rust-version"
	local opt__generate_lockfile="$opt_common $opt_mani $opt_lock"
	local opt__help="$opt_help"
	local opt__init="$opt_common $opt_lock --bin --lib --name --vcs --edition --registry"
	local opt__install="$opt_common $opt_feat $opt_parallel $opt_lock $opt_force --bin --bins --branch --debug --example --examples --git --list --path --rev --root --tag --version --registry --target --profile --no-track --ignore-rust-version"
	local opt__locate_project="$opt_common $opt_mani $opt_lock --message-format --workspace"
	local opt__login="$opt_common $opt_lock --registry"
	local opt__metadata="$opt_common $opt_feat $opt_mani $opt_lock --format-version=1 --no-deps --filter-platform"
	local opt__new="$opt_common $opt_lock --vcs --bin --lib --name --edition --registry"
	local opt__owner="$opt_common $opt_lock -a --add -r --remove -l --list --index --token --registry"
	local opt__package="$opt_common $opt_mani $opt_feat $opt_lock $opt_parallel --allow-dirty -l --list --no-verify --no-metadata --target --target-dir"
	local opt__pkgid="$opt_common $opt_mani $opt_lock $opt_pkg"
	local opt__publish="$opt_common $opt_mani $opt_feat $opt_lock $opt_parallel --allow-dirty --dry-run --token --no-verify --index --registry --target --target-dir"
	local opt__read_manifest="$opt_help $opt_quiet $opt_verbose $opt_mani $opt_color $opt_lock --no-deps"
	local opt__remove="$opt_common $opt_pkg $opt_lock $opt_mani --dry-run --dev --build --target"
	local opt__rm="$opt__remove"
	local opt__report="$opt_help $opt_verbose $opt_color future-incompat future-incompatibilities"
	local opt__report__future_incompat="$opt_help $opt_verbose $opt_color $opt_pkg --id"
	local opt__run="$opt_common $opt_pkg $opt_feat $opt_mani $opt_lock $opt_parallel --message-format --target --bin --example --release --target-dir --profile --ignore-rust-version"
	local opt__r="$opt__run"
	local opt__rustc="$opt_common $opt_pkg $opt_feat $opt_mani $opt_lock $opt_parallel $opt_targets -L --crate-type --extern --message-format --profile --target --release --target-dir --ignore-rust-version"
	local opt__rustdoc="$opt_common $opt_pkg $opt_feat $opt_mani $opt_lock $opt_parallel $opt_targets --message-format --target --release --open --target-dir --profile --ignore-rust-version"
	local opt__search="$opt_common $opt_lock --limit --index --registry"
	local opt__test="$opt_common $opt_pkg_spec $opt_feat $opt_mani $opt_lock $opt_jobs $opt_targets --message-format --doc --target --no-run --release --no-fail-fast --target-dir --profile --ignore-rust-version"
	local opt__t="$opt__test"
	local opt__tree="$opt_common $opt_pkg_spec $opt_feat $opt_mani $opt_lock --target -i --invert --prefix --no-dedupe --duplicates -d --charset -f --format -e --edges"
	local opt__uninstall="$opt_common $opt_lock $opt_pkg --bin --root"
	local opt__update="$opt_common $opt_mani $opt_lock $opt_pkg --aggressive --recursive --precise --dry-run"
	local opt__vendor="$opt_common $opt_mani $opt_lock $opt_sync --no-delete --respect-source-config --versioned-dirs"
	local opt__verify_project="$opt_common $opt_mani $opt_lock"
	local opt__version="$opt_common $opt_lock"
	local opt__yank="$opt_common $opt_lock --version --undo --index --token --registry"
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
			_ensure_cargo_commands_cache_filled
			COMPREPLY=( $( compgen -W "$__cargo_commands_cache" -- "$cur" ) )
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
			--target-dir|--path)
				_filedir -d
				;;
			help)
				_ensure_cargo_commands_cache_filled
				COMPREPLY=( $( compgen -W "$__cargo_commands_cache" -- "$cur" ) )
				;;
			*)
				if [[ "$cmd" == "report" && "$prev" == future-incompat* ]]; then
					local opt_var=opt__${cmd//-/_}__${prev//-/_}
				else
					local opt_var=opt__${cmd//-/_}
				fi
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

__cargo_commands_cache=
_ensure_cargo_commands_cache_filled(){
	if [[ -z $__cargo_commands_cache ]]; then
		__cargo_commands_cache="$(cargo --list 2>/dev/null | awk 'NR>1 {print $1}')"
	fi
}

_locate_manifest(){
	cargo locate-project --message-format plain 2>/dev/null
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
	if command -v rustup >/dev/null 2>/dev/null; then
	  rustup target list --installed
	else
	  rustc --print target-list
	fi
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
