% Crates.io package policies

# Packages Policy for Crates.io

In [a previous post to the Rust blog]
(http://blog.rust-lang.org/2014/11/20/Cargo.html), 
we announced the preview launch of
[crates.io](http://crates.io/), giving the Rust community a 
way to easily publish packages. After a few weeks of kicking the tires, and
hearing the most common questions people have about the registry, we wanted to
clarify the rationale behind some of the design decisions. We also wanted to
take the opportunity to be more explicit about the policies around package
ownership on crates.io.

In general, these policies are guidelines. Problems are often contextual, and
exceptional circumstances sometimes require exceptional measures. We plan to
continue to clarify and expand these rules over time as new circumstances arise.

# Package Ownership

We have had, and will continue to have, a first-come, first-served policy on
crate names. Upon publishing a package, the publisher will be made owner of the
package on Crates.io. This follows the precedent of nearly all package
management ecosystems.

# Removal

Many questions are specialized instances of a more general form: “Under what
circumstances can a package be removed from Crates.io?”

The short version is that packages are first-come, first-served, and we won’t
attempt to get into policing what exactly makes a legitimate package. We will do
what the law requires us to do, and address flagrant violations of the Rust Code
of Conduct.

# Squatting

Nobody likes a “squatter”, but finding good rules that define squatting that can
be applied mechanically is notoriously difficult. If we require that the package
has at least some content in it, squatters will insert random content. If we
require regular updates, squatters will make sure to update regularly, and that
rule might apply over-zealously to packages that are relatively stable.


A more case-by-case policy would be very hard to get right, and would almost
certainly result in bad mistakes and and regular controversies.

Instead, we are going to stick to a first-come, first-served system. If someone
wants to take over a package, and the previous owner agrees, the existing
maintainer can add them as an owner, and the new maintainer can remove them. If
necessary, the team may reach out to inactive maintainers and help mediate the
process of ownership transfer. We know that this means, in practice, that
certain desirable names will be taken early on, and that those early users may
not be using them in the most optimal way (whether they are claimed by squatters
or just low-quality packages). Other ecosystems have addressed this problem
through the use of more colorful names, and we think that this is actually a
feature, not a bug, of this system. We talk about this more below.

# The Law

For issues such as DMCA violations, trademark and copyright infringement,
Crates.io will respect Mozilla Legal’s decisions with regards to content that is
hosted.

# Code of Conduct

The Rust project has a [Code of Conduct]
(https://github.com/rust-lang/rust/wiki/Note-development-policy#conduct) 
which governs appropriate conduct for the Rust community. In general, any
content on Crates.io that violates the Code of Conduct may be removed. There are
two important, related aspects:

- We will not be pro-actively monitoring the site for these kinds of violations,
  but relying on the community to draw them to our attention.
- “Does this violate the Code of Conduct” is a contextual question that 
  cannot be directly answered in the hypothetical sense. All of the details 
  must be taken into consideration in these kinds of situations.

We plan on adding ‘report’ functionality to alert the administrators that a
package may be in violation of some of these rules.

# Namespacing

In the first month with crates.io, a number of people have asked us aboutthe
possibility of introducing [namespaced packages]
(https://github.com/rust-lang/crates.io/issues/58).

While namespaced packages allow multiple authors to use a single, generic name,
they add complexity to how packaged are referenced in Rust code and in human
communication about packages. At first glance, they allow multiple authors to
claim names like http, but that simply means that people will need to refer to
those packages as `wycats’ http or reem’s http`, offering little benefit over
package names like wycats-http or reem-http.

When we looked at package ecosystems without namespacing, we found that people
tended to go with more creative names (like nokogiri instead of “tenderlove’s
libxml2”). These creative names tend to be short and memorable, in part because
of the lack of any hierarchy. They make it easier to communicate concisely and
unambiguously about packages. They create exciting brands. And we’ve seen the
success of several 10,000+ package ecosystems like NPM and RubyGems whose
communities are prospering within a single namespace.

In short, we don’t think the Cargo ecosystem would be better off if Piston chose
a name like `bvssvni/game-engine` (allowing other users to choose
`wycats/game-engine`) instead of simply piston.

Because namespaces are strictly more complicated in a number of ways,and because
they can be added compatibly in the future should they become necessary, we’re
going to stick with a single shared namespace.

# Organizations & related packages

One situation in which a namespace could be useful is when an organization
releases a number of related packages. We plan on expanding the ’tags’ feature
to indicate when multiple crates come from one organization. Details about this
plan will come at a later time.
