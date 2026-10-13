# Put `git commit -m "<title>"` on the command line. Does not run the commit.
gitmsg() {
  local line
  line="$(command gitmsg --print "$@")" || return $?
  print -z -- "$line"
}
