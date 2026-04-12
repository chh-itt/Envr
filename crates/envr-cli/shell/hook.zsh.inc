# envr: directory hook for zsh — install with: eval "$(envr hook zsh)"
# Applies merged project env when you cd into a tree with .envr.toml; restores when you leave.
# Optional prompt: PS1='[$(_envr_prompt_segment)]%m:%~%# ' (see `envr hook prompt`).

_envr_hook_find_root() {
	local d="$PWD"
	while [[ "$d" != "/" ]]; do
		if [[ -f "$d/.envr.toml" || -f "$d/.envr.local.toml" ]]; then
			print -r -- "$d"
			return 0
		fi
		d="${d:h}"
	done
	return 1
}

_envr_hook_capture() {
	local root="$1" k
	typeset -ga _envr_hook_keys
	_envr_hook_keys=()
	unset _envr_hook_had _envr_hook_val 2>/dev/null || true
	typeset -gA _envr_hook_had _envr_hook_val
	while IFS= read -r k; do
		[[ -z "$k" ]] && continue
		_envr_hook_keys+=("$k")
		if (( ${+parameters[$k]} )); then
			_envr_hook_had[$k]=1
			_envr_hook_val[$k]=${(P)k}
		else
			_envr_hook_had[$k]=0
		fi
	done < <(envr hook keys --path "$root" 2>/dev/null)
}

_envr_hook_restore() {
	local k
	for k in $_envr_hook_keys; do
		if [[ ${_envr_hook_had[$k]} == 1 ]]; then
			export "${k}=${_envr_hook_val[$k]}"
		else
			unset "$k"
		fi
	done
	_envr_hook_keys=()
	unset _envr_hook_had _envr_hook_val 2>/dev/null || true
}

_envr_hook_main() {
	local root
	root="$(_envr_hook_find_root)" || root=""
	if [[ "${root:-}" == "${_ENVR_HOOK_ROOT:-}" ]]; then
		return 0
	fi
	if [[ -n "${_ENVR_HOOK_ROOT:-}" ]]; then
		_envr_hook_restore
	fi
	_ENVR_HOOK_ROOT="${root:-}"
	if [[ -z "${root:-}" ]]; then
		return 0
	fi
	_envr_hook_capture "$root"
	if [[ -n "${ENVR_HOOK_VERBOSE:-}" ]]; then
		echo "envr hook: applying $root" >&2
	fi
	eval "$(envr env --shell posix --path "$root" 2>/dev/null)" || true
}

# After `eval "$(envr hook zsh)"`, `envr deactivate` / `envr off` restores saved vars.
envr() {
	case "${1-}" in
	deactivate|off)
		_envr_hook_restore
		unset _ENVR_HOOK_ROOT 2>/dev/null || true
		;;
	*)
		command envr "$@"
		;;
	esac
}

_envr_prompt_segment() {
	command envr hook prompt 2>/dev/null | tr -d '\n'
}

autoload -Uz add-zsh-hook 2>/dev/null || true
if (( ${+functions[add-zsh-hook]} )); then
	add-zsh-hook -d chpwd _envr_hook_main 2>/dev/null || true
	add-zsh-hook chpwd _envr_hook_main
else
	chpwd_functions=(${chpwd_functions:#_envr_hook_main})
	chpwd_functions+=(_envr_hook_main)
fi
_envr_hook_main
