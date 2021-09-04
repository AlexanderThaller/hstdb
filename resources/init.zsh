function hstdb-init() {
  local session_id;
  session_id="$(hstdb session_id)"
  export HISTDB_RS_SESSION_ID="${session_id}"
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

hstdb-init
