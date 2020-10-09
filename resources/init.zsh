function histdb-rs-init() {
  local session_id;
  session_id="$(/home/athaller/.cargo/bin/histdb-rs session_id)"
  export HISTDB_RS_SESSION_ID="${session_id}"
}

function histdb-rs-zshaddhistory() {
  unset HISTDB_RS_RETVAL;
  /home/athaller/.cargo/bin/histdb-rs zshaddhistory $@
}

function histdb-rs-precmd() {
  export HISTDB_RS_RETVAL="${?}"
  /home/athaller/.cargo/bin/histdb-rs precmd
}

add-zsh-hook zshaddhistory histdb-rs-zshaddhistory
add-zsh-hook precmd histdb-rs-precmd

histdb-rs-init
