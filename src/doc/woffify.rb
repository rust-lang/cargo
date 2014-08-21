require "fileutils"

FileUtils.rm Dir["source/fonts/**/*.woff"]

Dir["source/fonts/**/*.ttf"].each do |file|
  out = file.sub(/\.ttf$/, ".woff").gsub(' ', "-")
  puts "Converting `#{file}` to `#{out}`"
  `~/npm/bin/ttf2woff "#{file}" "#{out}"`
end
