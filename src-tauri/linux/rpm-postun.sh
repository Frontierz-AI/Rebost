# RPM removes the files it installed but not the folders around them.
# $1 is 0 on uninstall and 1 or more on upgrade.
if [ "$1" = 0 ]; then
  for dir in /usr/lib/Rebost/resources/engine /usr/lib/Rebost/resources/tessdata /usr/lib/Rebost/resources /usr/lib/Rebost; do
    rmdir "$dir" 2>/dev/null || true
  done
fi
