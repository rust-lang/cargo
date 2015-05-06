command -v cargo >/dev/null 2>&1 &&
_cargo()
{
	local cur prev words cword cmd
	_init_completion || return

	COMPREPLY=()

	cmd=${words[1]}

	opt_common='-h --help -v --verbose'
	opt_pkg='-p --package'
	opt_feat='--features --no-default-features'
	opt_mani='--manifest-path'
	opt_jobs='-j --jobs'

	declare -A opts
	opts[_nocmd]="$opt_common -V --version --list"
	opts[bench]="$opt_common $opt_pkg $opt_feat $opt_mani $opt_jobs --target --lib --bin --test --bench --example --no-run"
	opts[build]="$opt_common $opt_pkg $opt_feat $opt_mani $opt_jobs --target --lib --bin --test --bench --example --release"
	opts[clean]="$opt_common $opt_pkg $opt_mani --target"
	opts[doc]="$opt_common $opt_pkg $opt_feat $opt_mani $opt_jobs --target --open --no-deps"
	opts[fetch]="$opt_common $opt_mani"
	opts[generate-lockfile]="${opts[fetch]}"
	opts[git-checkout]="$opt_common --reference= --url="
	opts[locate-project]="$opt_mani -h --help"
	opts[login]="$opt_common --host"
	opts[new]="$opt_common --vcs --bin --name"
	opts[owner]="$opt_common -a --add -r --remove -l --list --index --token"
	opts[pkgid]="${opts[fetch]}"
	opts[publish]="$opt_common $opt_mani --host --token --no-verify"
	opts[read-manifest]="${opts[fetch]}"
	opts[run]="$opt_common $opt_feat $opt_mani $opt_jobs --target --bin --example --release"
	opts[test]="$opt_common $opt_pkg $opt_feat $opt_mani $opt_jobs --target --lib --bin --test --bench --example --no-run --release"
	opts[update]="$opt_common $opt_pkg $opt_mani --aggressive --precise"
	opts[package]="$opt_common $opt_mani -l --list --no-verify --no-metadata"
	opts[verify-project]="${opts[fetch]}"
	opts[version]="$opt_common"
	opts[yank]="$opt_common --vers --undo --index --token"

	if [[ $cword -eq 1 ]]; then
		if [[ "$cur" == -* ]]; then
			COMPREPLY=( $( compgen -W "${opts[_nocmd]}" -- "$cur" ) )
		else
			COMPREPLY=( $( compgen -W "$(cargo --list | tail -n +2)" -- "$cur" ) )
		fi
	elif [[ $cword -ge 2 ]]; then
		case "${prev}" in
			--manifest-path)
				_filedir toml
				;;
			--example)
				COMPREPLY=( $( compgen -W "$(_get_examples)" -- "$cur" ) )
				;;
			*)
				COMPREPLY=( $( compgen -W "${opts[$cmd]}" -- "$cur" ) )
				;;
		esac
	fi

	if [[ ${#COMPREPLY[@]} == 1 && ${COMPREPLY[0]} != "--"*"=" ]] ; then
		compopt +o nospace
	fi
	return 0
} &&
complete -o nospace -F _cargo cargo

_locate_manifest(){
	local manifest=`cargo locate-project 2>/dev/null`
	# regexp-replace manifest '\{"root":"|"\}' ''
	echo ${manifest:9:-2}
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
# vim:ft=sh
