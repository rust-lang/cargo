# resolver-tests

## The aim

This crate aims to test the resolution of Cargo's resolver. It implements a [SAT solver](https://en.wikipedia.org/wiki/SAT_solver) to compare with resolution of Cargo's resolver.    
This ensures that Cargo's dependency resolution is proven valid by lowering to [SAT problem](https://en.wikipedia.org/wiki/Boolean_satisfiability_problem). 

## About the test

The Cargo's resolver is very sensitive to what order it tries to evaluate constraints. This makes it incredibly difficult     
to be sure that a handful of tests actually covers all the important permutations of decision-making. The tests not only needs    
to hit all the corner cases, it needs to try all of the orders of evaluation. So we use fuzz testing to cover more permutations.    

