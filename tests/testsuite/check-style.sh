echo "checking for lines over 100 characters..."
find src tests -name '*.rs' | xargs grep '.\{101,\}' && exit 1
echo "ok"
