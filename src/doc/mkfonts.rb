require "erb"

def font(url, family, weight: weight, italic: italic)
  url = "../fonts/#{url}.woff"

  erb = ERB.new(<<-HERE.gsub(/^    /, ""), nil, "-")
    @font-face {
      src: url("#{url}");
      font-family: "<%= family %>";
      <%- if weight -%>
      font-weight: <%= weight %>;
      <%- end -%>
      <%- if italic -%>
      font-style: italic;
      <%- end -%>
    }

  HERE

  erb.result(binding)
end

File.open("source/stylesheets/fonts.css.scss", "w") do |file|
  file.puts font("Consolas", "Consolas")
  file.puts font("Consolas-Bold", "Consolas", weight: "bold")
  file.puts font("Consolas-Italic", "Consolas", italic: true)
  file.puts font("Consolas-Bold-Italic", "Consolas", weight: "bold", italic: true)

  file.puts font("Roboto/Roboto-Thin", "Roboto", weight: 100)
  file.puts font("Roboto/Roboto-Light", "Roboto", weight: 200)
  file.puts font("Roboto/Roboto-Regular", "Roboto", weight: 400)
  file.puts font("Roboto/Roboto-Medium", "Roboto", weight: 500)
  file.puts font("Roboto/Roboto-Bold", "Roboto", weight: 700)
  file.puts font("Roboto/Roboto-Black", "Roboto", weight: 800)
end
