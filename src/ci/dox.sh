set -ex

DOCS="index faq config guide manifest build-script pkgid-spec crates-io \
      environment-variables specifying-dependencies source-replacement \
      policies external-tools"
ASSETS="CNAME images/noise.png images/forkme.png images/Cargo-Logo-Small.png \
        stylesheets/all.css stylesheets/normalize.css javascripts/prism.js \
        javascripts/all.js stylesheets/prism.css images/circle-with-i.png \
        images/search.png images/org-level-acl.png images/auth-level-acl.png \
        favicon.ico"

for asset in $ASSETS; do
  mkdir -p `dirname target/doc/$asset`
  cp src/doc/$asset target/doc/$asset
done

for doc in $DOCS; do
  rustdoc \
    --markdown-no-toc \
    --markdown-css stylesheets/normalize.css \
    --markdown-css stylesheets/all.css \
    --markdown-css stylesheets/prism.css \
    --html-in-header src/doc/html-headers.html \
    --html-before-content src/doc/header.html \
    --html-after-content src/doc/footer.html \
    -o target/doc \
    src/doc/$doc.md
done
