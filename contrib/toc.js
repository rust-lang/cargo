// Populate the sidebar
//
// This is a script, and not included directly in the page, to control the total size of the book.
// The TOC contains an entry for each page, so if each page includes a copy of the TOC,
// the total size of the page becomes O(n**2).
class MDBookSidebarScrollbox extends HTMLElement {
    constructor() {
        super();
    }
    connectedCallback() {
        this.innerHTML = '<ol class="chapter"><li class="chapter-item expanded "><a href="index.html"><strong aria-hidden="true">1.</strong> Introduction</a></li><li class="chapter-item expanded "><a href="issues.html"><strong aria-hidden="true">2.</strong> Issue Tracker</a></li><li class="chapter-item expanded "><a href="team.html"><strong aria-hidden="true">3.</strong> Cargo Team</a></li><li class="chapter-item expanded "><a href="process/index.html"><strong aria-hidden="true">4.</strong> Process</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="process/working-on-cargo.html"><strong aria-hidden="true">4.1.</strong> Working on Cargo</a></li><li class="chapter-item expanded "><a href="process/release.html"><strong aria-hidden="true">4.2.</strong> Release process</a></li><li class="chapter-item expanded "><a href="process/rfc.html"><strong aria-hidden="true">4.3.</strong> Writing an RFC</a></li><li class="chapter-item expanded "><a href="process/unstable.html"><strong aria-hidden="true">4.4.</strong> Unstable features</a></li><li class="chapter-item expanded "><a href="process/security.html"><strong aria-hidden="true">4.5.</strong> Security issues</a></li></ol></li><li class="chapter-item expanded "><a href="design.html"><strong aria-hidden="true">5.</strong> Design Principles</a></li><li class="chapter-item expanded "><a href="implementation/index.html"><strong aria-hidden="true">6.</strong> Implementing a Change</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="implementation/architecture.html"><strong aria-hidden="true">6.1.</strong> Architecture</a></li><li class="chapter-item expanded "><a href="implementation/packages.html"><strong aria-hidden="true">6.2.</strong> New packages</a></li><li class="chapter-item expanded "><a href="implementation/subcommands.html"><strong aria-hidden="true">6.3.</strong> New subcommands</a></li><li class="chapter-item expanded "><a href="implementation/schemas.html"><strong aria-hidden="true">6.4.</strong> Data Schemas</a></li><li class="chapter-item expanded "><a href="implementation/console.html"><strong aria-hidden="true">6.5.</strong> Console Output</a></li><li class="chapter-item expanded "><a href="implementation/filesystem.html"><strong aria-hidden="true">6.6.</strong> Filesystem</a></li><li class="chapter-item expanded "><a href="implementation/formatting.html"><strong aria-hidden="true">6.7.</strong> Formatting</a></li><li class="chapter-item expanded "><a href="implementation/debugging.html"><strong aria-hidden="true">6.8.</strong> Debugging</a></li></ol></li><li class="chapter-item expanded "><a href="tests/index.html"><strong aria-hidden="true">7.</strong> Tests</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="tests/running.html"><strong aria-hidden="true">7.1.</strong> Running Tests</a></li><li class="chapter-item expanded "><a href="tests/writing.html"><strong aria-hidden="true">7.2.</strong> Writing Tests</a></li><li class="chapter-item expanded "><a href="tests/profiling.html"><strong aria-hidden="true">7.3.</strong> Benchmarking and Profiling</a></li><li class="chapter-item expanded "><a href="tests/crater.html"><strong aria-hidden="true">7.4.</strong> Crater</a></li></ol></li><li class="chapter-item expanded "><a href="documentation/index.html"><strong aria-hidden="true">8.</strong> Documentation</a></li></ol>';
        // Set the current, active page, and reveal it if it's hidden
        let current_page = document.location.href.toString().split("#")[0].split("?")[0];
        if (current_page.endsWith("/")) {
            current_page += "index.html";
        }
        var links = Array.prototype.slice.call(this.querySelectorAll("a"));
        var l = links.length;
        for (var i = 0; i < l; ++i) {
            var link = links[i];
            var href = link.getAttribute("href");
            if (href && !href.startsWith("#") && !/^(?:[a-z+]+:)?\/\//.test(href)) {
                link.href = path_to_root + href;
            }
            // The "index" page is supposed to alias the first chapter in the book.
            if (link.href === current_page || (i === 0 && path_to_root === "" && current_page.endsWith("/index.html"))) {
                link.classList.add("active");
                var parent = link.parentElement;
                if (parent && parent.classList.contains("chapter-item")) {
                    parent.classList.add("expanded");
                }
                while (parent) {
                    if (parent.tagName === "LI" && parent.previousElementSibling) {
                        if (parent.previousElementSibling.classList.contains("chapter-item")) {
                            parent.previousElementSibling.classList.add("expanded");
                        }
                    }
                    parent = parent.parentElement;
                }
            }
        }
        // Track and set sidebar scroll position
        this.addEventListener('click', function(e) {
            if (e.target.tagName === 'A') {
                sessionStorage.setItem('sidebar-scroll', this.scrollTop);
            }
        }, { passive: true });
        var sidebarScrollTop = sessionStorage.getItem('sidebar-scroll');
        sessionStorage.removeItem('sidebar-scroll');
        if (sidebarScrollTop) {
            // preserve sidebar scroll position when navigating via links within sidebar
            this.scrollTop = sidebarScrollTop;
        } else {
            // scroll sidebar to current active section when navigating via "next/previous chapter" buttons
            var activeSection = document.querySelector('#sidebar .active');
            if (activeSection) {
                activeSection.scrollIntoView({ block: 'center' });
            }
        }
        // Toggle buttons
        var sidebarAnchorToggles = document.querySelectorAll('#sidebar a.toggle');
        function toggleSection(ev) {
            ev.currentTarget.parentElement.classList.toggle('expanded');
        }
        Array.from(sidebarAnchorToggles).forEach(function (el) {
            el.addEventListener('click', toggleSection);
        });
    }
}
window.customElements.define("mdbook-sidebar-scrollbox", MDBookSidebarScrollbox);
