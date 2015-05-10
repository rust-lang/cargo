command -v cargo >/dev/null 2>&1 &&
_cargo()
{
	local cur prev words cword cmd
	_get_comp_words_by_ref cur prev words cword

	COMPREPLY=()

	cmd=${words[1]}

	local opt_common='-h --help -v --verbose'
	local opt_pkg='-p --package'
	local opt_feat='--features --no-default-features'
	local opt_mani='--manifest-path'
	local opt_jobs='-j --jobs'

	local opt___nocmd="$opt_common -V --version --list"
	local opt__bench="$opt_common $opt_pkg $opt_feat $opt_mani $opt_jobs --target --lib --bin --test --bench --example --no-run"
	local opt__build="$opt_common $opt_pkg $opt_feat $opt_mani $opt_jobs --target --lib --bin --test --bench --example --release"
	local opt__clean="$opt_common $opt_pkg $opt_mani --target"
	local opt__doc="$opt_common $opt_pkg $opt_feat $opt_mani $opt_jobs --target --open --no-deps"
	local opt__fetch="$opt_common $opt_mani"
	local opt__generate_lockfile="${opt__fetch}"
	local opt__git_checkout="$opt_common --reference= --url="
	local opt__locate_project="$opt_mani -h --help"
	local opt__login="$opt_common --host"
	local opt__new="$opt_common --vcs --bin --name"
	local opt__owner="$opt_common -a --add -r --remove -l --list --index --token"
	local opt__pkgid="${opt__fetch}"
	local opt__publish="$opt_common $opt_mani --host --token --no-verify"
	local opt__read_manifest="${opt__fetch}"
	local opt__run="$opt_common $opt_feat $opt_mani $opt_jobs --target --bin --example --release"
	local opt__test="$opt_common $opt_pkg $opt_feat $opt_mani $opt_jobs --target --lib --bin --test --bench --example --no-run --release"
	local opt__update="$opt_common $opt_pkg $opt_mani --aggressive --precise"
	local opt__package="$opt_common $opt_mani -l --list --no-verify --no-metadata"
	local opt__verify_project="${opt__fetch}"
	local opt__version="$opt_common"
	local opt__yank="$opt_common --vers --undo --index --token"

	if [[ $cword -eq 1 ]]; then
		if [[ "$cur" == -* ]]; then
			COMPREPLY=( $( compgen -W "${opt___nocmd}" -- "$cur" ) )
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
				local opt_var=opt__${cmd//-/_}
				COMPREPLY=( $( compgen -W "${!opt_var}" -- "$cur" ) )
				;;
		esac
	fi

	# compopt does not work in bash version 3

	return 0
} &&
complete -F _cargo cargo

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
