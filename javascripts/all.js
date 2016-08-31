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

    $('button.dropdown, a.dropdown').click(function(el, e) {
        $(this).toggleClass('active');
        $(this).siblings('ul').toggleClass('open');

        if ($(this).hasClass('active')) {
            $(document).on('mousedown.useroptions', function() {
                setTimeout(function() {
                    $('button.dropdown, a.dropdown').removeClass('active');
                    $('button.dropdown + ul').removeClass('open');
                }, 150);
                $(document).off('mousedown.useroptions');
            });
        }
    });
});
