//= require_tree .

Prism.languages.toml = {
  'string': /("|')(\\?.)*?\1/g,
  'comment': /#.*/,
  // 'atrule': {
  //   pattern: /@[\w-]+?.*?(;|(?=\s*{))/gi,
  //   inside: {
  //     'punctuation': /[;:]/g
  //   }
  // },
  // 'url': /url\((["']?).*?\1\)/gi,
  // 'selector': /[^\{\}\s][^\{\};]*(?=\s*\{)/g,
  // 'property': /(\b|\B)[\w-]+(?=\s*:)/ig,
  // 'punctuation': /[\{\};:]/g,
  // 'function': /[-a-z0-9]+(?=\()/ig
  'number': /\d+/,
  'boolean': /true|false/,
  'toml-section': /\[.*\]/,
  'toml-key': /[\w-]+/
};

(function() {
  var pres = document.querySelectorAll('pre.rust');
  for (var i = 0; i < pres.length; i++) {
    pres[i].className += ' language-rust';
  }
})();
