#!/usr/bin/env python3

MAPPING = {
    "build-script.html": "https://doc.rust-lang.org/cargo/reference/build-scripts.html",
    "config.html": None,
    "crates-io.html": "https://doc.rust-lang.org/cargo/reference/publishing.html",
    "environment-variables.html": None,
    "external-tools.html": None,
    "faq.html": "https://doc.rust-lang.org/cargo/faq.html",
    "guide.html": "https://doc.rust-lang.org/cargo/guide/",
    "index.html": "https://doc.rust-lang.org/cargo/",
    "manifest.html": None,
    "pkgid-spec.html": None,
    "policies.html": "https://crates.io/policies",
    "source-replacement.html": None,
    "specifying-dependencies.html": None,
}

TEMPLATE = """\
<html>
<head>
<meta http-equiv="refresh" content="0; url={mapped}" />
<script>
window.location.replace("{mapped}" + window.location.hash);
</script>
<title>Page Moved</title>
</head>
<body>
This page has moved. Click <a href="{mapped}">here</a> to go to the new page.
</body>
</html>
"""

def main():
    for name in sorted(MAPPING):
        with open(name, 'w') as f:
            mapped = MAPPING[name]
            if mapped is None:
                mapped = "https://doc.rust-lang.org/cargo/reference/{}".format(name)
            f.write(TEMPLATE.format(name=name, mapped=mapped))

    # WARN: The CNAME file is for GitHub to redirect requests to the custom domain.
    # Missing this may entail security hazard and domain takeover.
    # See <https://docs.github.com/en/pages/configuring-a-custom-domain-for-your-github-pages-site/managing-a-custom-domain-for-your-github-pages-site#securing-your-custom-domain>
    with open('CNAME', 'w') as f:
        f.write('doc.crates.io')

if __name__ == '__main__':
    main()
