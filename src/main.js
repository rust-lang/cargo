/// Just a prototype, the final thing should be in Rust

const fs = require('fs');
const args = process.argv.slice(2);

if (!args[0]) {
    console.log("Usage: node src/main.js json-file");
    process.exit(1);
}

/// Strips the indent of the first line form all other lines
function normalizeIndent(lines) {
    if (!lines || !lines[0]) { return []; }
    const matches = lines[0].match(/^(\s*)(.*)$/);
    const leadingWhitespace = matches[1];

    return lines
        .map(line => line.replace(new RegExp("^" + leadingWhitespace), ""));
}

function collectSpan(acc, message, span) {
    if (!span.suggested_replacement) { return; }
    acc.push({
        message: message,
        file_name: span.file_name,
        range: [
            [span.line_start, span.column_start],
            [span.line_end, span.column_end],
        ],
        text: normalizeIndent((span.text || []).map(x => x.text)),
        replacement: span.suggested_replacement,
    })
}

function collectSuggestions(acc, diagnostic, parent_message) {
    const message = typeof parent_message === 'string' ?
        parent_message :
        diagnostic.message;

    (diagnostic.spans || [])
        .forEach(span => collectSpan(acc, message, span));

    (diagnostic.children || [])
        .forEach(child => collectSuggestions(acc, child, message));

    return acc;
}


const file = fs.readFileSync(args[0]).toString('utf8');
const lines = file.split('\n');
const diagnostics = lines
    .filter(line => line.trim().length > 0)
    .map(line => JSON.parse(line));
const suggestions = diagnostics
    .reduce(collectSuggestions, []);

suggestions.forEach(suggestion => {
    console.log("___________________________________________");
    console.log("");
    console.log(suggestion.message);
    console.log("");
    console.log("You might want to replace");
    console.log("");
    console.log(suggestion.text
        .map(line => "-   " + line).join("\n"));
    console.log("");
    console.log("with");
    console.log("");
    console.log("+   " + suggestion.replacement);
    console.log("");
});
