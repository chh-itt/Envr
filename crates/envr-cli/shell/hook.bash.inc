# shellcheck shell=bash
# envr: directory hook for bash — install with: eval "$(envr hook bash)"
# Bumps PATH / project [env] when you cd into a tree that contains .envr.toml, and restores when you leave.
# Optional prompt: PS1='[$(_envr_prompt_segment)]\u@\h:\w\$ ' (text-only segment from `envr hook prompt`).
# Requires bash 4+ (associative arrays). On macOS, prefer zsh (eval "$(envr hook zsh)") or brew bash.

_envr_hook_find_root() {
	local d="$PWD"
	while [[ "$d" != "/" ]]; do
		if [[ -f "$d/.envr.toml" || -f "$d/.envr.local.toml" ]]; then
			printf '%s\n' "$d"
			return 0
		fi
		d="$(dirname "$d")"
	done
	return 1
}

_envr_hook_capture() {
	local root="$1" k
	_ENVR_HOOK_KEYS=()
	unset _ENVR_HOOK_HAD 2>/dev/null || true
	unset _ENVR_HOOK_VAL 2>/dev/null || true
	declare -gA _ENVR_HOOK_HAD
	declare -gA _ENVR_HOOK_VAL
	while IFS= read -r k; do
		[[ -z "$k" ]] && continue
		_ENVR_HOOK_KEYS+=("$k")
		if eval "[[ -n \"\${$k+x}\" ]]"; then
			_ENVR_HOOK_HAD[$k]=1
			_ENVR_HOOK_VAL[$k]="$(eval "printf '%s' \"\$$k\"")"
		else
			_ENVR_HOOK_HAD[$k]=0
		fi
	done < <(envr hook keys --path "$root" 2>/dev/null)
}

_envr_hook_restore() {
	local k
	for k in "${_ENVR_HOOK_KEYS[@]}"; do
		if [[ "${_ENVR_HOOK_HAD[$k]}" == 1 ]]; then
			export "$k=${_ENVR_HOOK_VAL[$k]}"
		else
			unset "$k"
		fi
	done
	_ENVR_HOOK_KEYS=()
	unset _ENVR_HOOK_HAD 2>/dev/null || true
	unset _ENVR_HOOK_VAL 2>/dev/null || true
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

# After `eval "$(envr hook bash)"`, `envr deactivate` / `envr off` restores saved vars (`command envr` = real binary).
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

if [[ -n "${BASH_VERSION:-}" ]]; then
	if [[ "${PROMPT_COMMAND:-}" != *"_envr_hook_main"* ]]; then
		PROMPT_COMMAND="_envr_hook_main${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
	fi
	_envr_hook_main
fi
