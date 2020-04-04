require 'asciidoctor/extensions' unless RUBY_ENGINE == 'opal'

include Asciidoctor

# An inline macro that generates links to related man pages.
#
# Usage
#
#   man:gittutorial[7]
#
class ManInlineMacro < Extensions::InlineMacroProcessor
  use_dsl

  named :man
  name_positional_attributes 'volnum'

  def process parent, target, attrs
    manname = target
    suffix = if (volnum = attrs['volnum'])
      "(#{volnum})"
    else
      nil
    end
    text = %(#{manname}#{suffix})
    if parent.document.basebackend? 'html'
      parent.document.register :links, target
      if manname == 'rustc'
        html_target = 'https://doc.rust-lang.org/rustc/index.html'
      elsif manname == 'rustdoc'
        html_target = 'https://doc.rust-lang.org/rustdoc/index.html'
      elsif manname == 'cargo'
        html_target = 'index.html'
      else
        html_target = %(#{manname}.html)
      end
      %(#{(create_anchor parent, text, type: :link, target: html_target).render})
    elsif parent.document.backend == 'manpage'
      %(\x1b\\fB#{manname}\x1b\\fP#{suffix})
    else
      text
    end
  end
end

# Creates a link to something in the cargo documentation.
#
# For HTML this creates a relative link. For the man page it gives a direct
# link to doc.rust-lang.org.
#
# Usage
#
#   linkcargo:reference/manifest.html[the manifest]
#
class LinkCargoInlineMacro < Extensions::InlineMacroProcessor
  use_dsl

  named :linkcargo
  name_positional_attributes 'text'

  def process parent, target, attrs
    text = attrs['text']
    if parent.document.basebackend? 'html'
      target = %(../#{target})
      parent.document.register :links, target
      %(#{(create_anchor parent, text, type: :link, target: target).render})
    elsif parent.document.backend == 'manpage'
      target = %(https://doc.rust-lang.org/cargo/#{target})
      %(#{(create_anchor parent, text, type: :link, target: target).render})
    else
      %(#{text} <#{target}>)
    end
  end
end

# Backticks in the manpage renderer use the CR font (courier), but in most
# cases in a terminal this doesn't look any different. Instead, use bold which
# should follow man page conventions better.
class MonoPostprocessor < Extensions::Postprocessor
  def process document, output
    if document.basebackend? 'manpage'
      output = output.gsub(/\\f\(CR/, '\\fB')
    end
    output
  end
end

# Man pages are ASCII only. Unfortunately asciidoc doesn't process these
# characters for us. The `cargo tree` manpage needs a little assistance.
class SpecialCharPostprocessor < Extensions::Postprocessor
  def process document, output
    if document.basebackend? 'manpage'
      output = output.gsub(/│/, '|')
        .gsub(/├/, '|')
        .gsub(/└/, '`')
        .gsub(/─/, '\-')
    end
    output
  end
end

# General utility for converting text. Example:
#
#   convert:lowercase[{somevar}]
class ConvertInlineMacro < Extensions::InlineMacroProcessor
  use_dsl

  named :convert
  name_positional_attributes 'text'

  def process parent, target, attrs
    text = attrs['text']
    case target
    when 'lowercase'
      text.downcase
    end
  end
end

Extensions.register :uri_schemes do
  inline_macro ManInlineMacro
  inline_macro LinkCargoInlineMacro
  inline_macro ConvertInlineMacro
  postprocessor MonoPostprocessor
  postprocessor SpecialCharPostprocessor
end
