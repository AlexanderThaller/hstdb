function hstdb-init() {
  local session_id;
  session_id="$(hstdb session_id)"
  export HISTDB_RS_SESSION_ID="${session_id}"
}

function hstdb-restore-cursor() {
  if (( ${+commands[tput]} )); then
    tput cnorm
  fi
}

function hstdb-history-widget() {
  emulate -L zsh
  setopt localoptions noglobsubst noposixbuiltins pipefail no_aliases 2> /dev/null

  local selected
  local -a history_args skim_args

  history_args=(
    --entries-count "${HSTDB_SKIM_HISTORY_ENTRIES_COUNT:-10000}"
    --latest-first
    --disable-formatting
    --hide-header
  )

  if [[ -n "${HSTDB_SKIM_HISTORY_ARGS:-}" ]]; then
    history_args+=("${(z)HSTDB_SKIM_HISTORY_ARGS}")
  fi

  skim_args=(
    --height=40%
    --layout=reverse
    --delimiter='\t'
    --nth=2..
    --no-multi
    --no-sort
    --scheme=history
    --bind=ctrl-r:toggle-sort
    --query "${LBUFFER}"
  )

  if [[ -n "${HSTDB_SKIM_CTRL_R_OPTS:-}" ]]; then
    skim_args+=("${(z)HSTDB_SKIM_CTRL_R_OPTS}")
  fi

  selected="$(hstdb "${history_args[@]}" | sk "${skim_args[@]}")" || {
    zle reset-prompt
    hstdb-restore-cursor
    return 0
  }

  selected="${selected##*$'\t'}"

  if [[ -n "${selected}" ]]; then
    BUFFER="${selected}"
    CURSOR=${#BUFFER}
  fi

  zle reset-prompt
  hstdb-restore-cursor
}

function hstdb-zshaddhistory() {
  unset HISTDB_RS_RETVAL;
  hstdb zshaddhistory $@
}

function hstdb-precmd() {
  export HISTDB_RS_RETVAL="${?}"
  hstdb precmd
}

autoload -Uz add-zsh-hook

add-zsh-hook zshaddhistory hstdb-zshaddhistory
add-zsh-hook precmd hstdb-precmd

if [[ -o interactive ]] && (( ${+commands[sk]} )); then
  zle -N hstdb-history-widget
  bindkey -M emacs '^R' hstdb-history-widget
  bindkey -M viins '^R' hstdb-history-widget
fi

hstdb-init
