//= require_tree .

Prism.languages.toml = {
    // https://github.com/LeaVerou/prism/issues/307
    'comment': [{
        pattern: /(^[^"]*?("[^"]*?"[^"]*?)*?[^"\\]*?)(\/\*[\w\W]*?\*\/|(^|[^:])#.*?(\r?\n|$))/g,
        lookbehind: true
    }],
    'string': /("|')(\\?.)*?\1/g,
    'number': /\d+/,
    'boolean': /true|false/,
    'toml-section': /\[.*\]/,
    'toml-key': /[\w-]+/
};

$(function() {
    var pres = document.querySelectorAll('pre.rust');
    for (var i = 0; i < pres.length; i++) {
        pres[i].className += ' language-rust';
    }

    // Toggles docs menu
    $('button.dropdown, a.dropdown').click(function(el, e) {
        $(this).toggleClass('active').siblings('ul').toggleClass('open');

        return false;
    });

    // A click in the page anywhere but in the menu will turn the menu off
    $(document).on('click', function(e) {
        // Checks to make sure the click did not come from inside dropdown menu
        // if it doesn't we close the menu
        // else, we do nothing and just follow the link
        if (!$(e.target).closest('ul.dropdown').length) {
            var toggles = $('button.dropdown.active, a.dropdown.active');
            toggles.toggleClass('active').siblings('ul').toggleClass('open');

        }
    });
});
